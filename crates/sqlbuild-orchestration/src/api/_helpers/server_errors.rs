//! Axum's own 5xx rejections are plain text naming Rust types; clients get the generic JSON error instead.

use axum::http::header;
use axum::response::{IntoResponse, Response};

use crate::api::errors::ApiError;

pub(crate) async fn generic_server_errors(response: Response) -> Response {
    let plain = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("text/plain"));
    if !(plain && response.status().is_server_error()) {
        return response;
    }
    ApiError {
        status: response.status(),
        message: "internal server error".into(),
        decision_id: None,
    }
    .into_response()
}
