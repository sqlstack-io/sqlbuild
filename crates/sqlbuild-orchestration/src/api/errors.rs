//! HTTP errors name the cause when the control database is unavailable; any other error never returns database diagnostics, connection strings or credentials.

use axum::Json;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use uuid::Uuid;

use crate::api::models::{ApiErrorBody, ErrorReply};
use crate::health::main::control_cause::control_cause;
use crate::health::models::{ControlCause, ControlTarget};
use crate::store::errors::ControlError;

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("invalid API connection: {0}")]
    Configuration(&'static str),
    #[error("API request failed: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("API rejected the request with HTTP {0}")]
    Rejected(reqwest::StatusCode),
}

#[derive(Debug)]
pub struct ApiError {
    pub(crate) status: StatusCode,
    pub(crate) message: String,
    pub(crate) decision_id: Option<Uuid>,
    /// The control database failure behind it, which the response names and the server's health records.
    pub(crate) cause: Option<ControlCause>,
}

impl From<ControlError> for ApiError {
    fn from(error: ControlError) -> Self {
        let cause: Option<ControlCause> = control_cause(&error, (&ControlTarget::default(), false))
            .filter(ControlCause::unavailable);
        if let Some(cause) = cause {
            return Self {
                status: StatusCode::SERVICE_UNAVAILABLE,
                message: cause.message.clone(),
                decision_id: None,
                cause: Some(cause),
            };
        }
        if error.transient() {
            return Self {
                status: StatusCode::SERVICE_UNAVAILABLE,
                message: "control store is busy; retry the request".into(),
                decision_id: None,
                cause: None,
            };
        }
        let plain = |status: StatusCode, message: String, decision_id: Option<Uuid>| Self {
            status,
            message,
            decision_id,
            cause: None,
        };
        match error {
            ControlError::Invalid(message) => plain(StatusCode::BAD_REQUEST, message, None),
            ControlError::NotCheckingIn(message) => plain(StatusCode::CONFLICT, message, None),
            ControlError::Denied {
                reason,
                decision_id,
            } => plain(
                StatusCode::FORBIDDEN,
                format!("{reason}; decision {decision_id}"),
                Some(decision_id),
            ),
            ControlError::UnreadableSnapshot { .. }
            | ControlError::StaleGeneration
            | ControlError::Fenced => plain(StatusCode::CONFLICT, error.to_string(), None),
            _ => plain(
                StatusCode::INTERNAL_SERVER_ERROR,
                "control-store operation failed".to_owned(),
                None,
            ),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ApiErrorBody {
            error: self.message,
            decision_id: self.decision_id,
            request_id: None,
            cause: None,
        };
        let mut response = (self.status, Json(body.clone())).into_response();
        response.extensions_mut().insert(ErrorReply {
            body,
            cause: self.cause,
        });
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        if self.status == StatusCode::UNAUTHORIZED {
            response
                .headers_mut()
                .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
        }
        response
    }
}
