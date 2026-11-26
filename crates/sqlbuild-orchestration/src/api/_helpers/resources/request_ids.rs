//! Give every request an ID, and finish each error response with it and with its control database cause.

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, header};
use axum::middleware::Next;
use axum::response::Response;
use chrono::Utc;
use uuid::Uuid;

use crate::api::constants::REQUEST_ID_HEADER;
use crate::api::models::{ErrorCause, ErrorReply, RequestId};
use crate::health::main::log_cause::log_cause;
use crate::health::models::ServerHealth;

/// Name every request in `x-request-id`; an error the control database's unavailability caused also names it, its cause and since when in the body, and is recorded and logged.
pub(crate) async fn identify_request(
    State(health): State<ServerHealth>,
    mut request: Request,
    next: Next,
) -> Response {
    let request_id: Uuid = Uuid::now_v7();
    request.extensions_mut().insert(RequestId(request_id));
    let mut response = next.run(request).await;
    if let Ok(value) = HeaderValue::from_str(&request_id.to_string()) {
        response.headers_mut().insert(REQUEST_ID_HEADER, value);
    }
    let Some(ErrorReply {
        mut body,
        cause: Some(cause),
    }) = response.extensions_mut().remove::<ErrorReply>()
    else {
        return response;
    };
    let cause = health.locate(cause);
    let since = health.control_failed(&cause, (Utc::now(), Some(request_id)));
    log_cause(Some(request_id), &cause);
    body.request_id = Some(request_id);
    body.error.clone_from(&cause.message);
    body.cause = Some(ErrorCause {
        case: cause.case,
        since,
    });
    let Ok(bytes) = serde_json::to_vec(&body) else {
        return response;
    };
    let (mut parts, _) = response.into_parts();
    parts.headers.remove(header::CONTENT_LENGTH);
    Response::from_parts(parts, Body::from(bytes))
}
