//! HTTP errors never return database diagnostics, connection strings, or credentials.

use axum::Json;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};

use crate::api::models::ApiErrorBody;
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
}

impl From<ControlError> for ApiError {
    fn from(error: ControlError) -> Self {
        match error {
            ControlError::Invalid(message) => Self {
                status: StatusCode::BAD_REQUEST,
                message,
            },
            ControlError::StaleGeneration | ControlError::Fenced => Self {
                status: StatusCode::CONFLICT,
                message: error.to_string(),
            },
            _ => Self {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                message: "control-store operation failed".into(),
            },
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut response = (
            self.status,
            Json(ApiErrorBody {
                error: self.message,
            }),
        )
            .into_response();
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
