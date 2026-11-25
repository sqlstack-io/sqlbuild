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
use crate::health::_helpers::log_cause::log_cause;
use crate::health::models::ServerHealth;

/// Name the request in its response's `x-request-id`; an error names it in its body too, and a control database
/// error names the database, is recorded in `health` and written to the server log.
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
    let Some(ErrorReply { mut body, cause }) = response.extensions_mut().remove::<ErrorReply>()
    else {
        return response;
    };
    body.request_id = Some(request_id);
    if let Some(cause) = cause {
        let cause = health.locate(cause);
        let now = Utc::now();
        let since = if cause.unavailable() {
            health.control_failed(&cause, (now, Some(request_id)))
        } else {
            now
        };
        log_cause(Some(request_id), &cause);
        body.error.clone_from(&cause.message);
        body.cause = Some(ErrorCause {
            case: cause.case,
            since,
        });
    }
    let Ok(bytes) = serde_json::to_vec(&body) else {
        return response;
    };
    let (mut parts, _) = response.into_parts();
    parts.headers.remove(header::CONTENT_LENGTH);
    Response::from_parts(parts, Body::from(bytes))
}
