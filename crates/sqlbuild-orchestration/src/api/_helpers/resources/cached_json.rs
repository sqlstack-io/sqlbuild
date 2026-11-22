//! Held JSON bodies served with their entity tag, or 304 when the request already names it.

use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};

use crate::api::constants::ANY_ETAG;
use crate::api::errors::ApiError;
use crate::node_states::models::CachedJson;
use crate::store::errors::ControlError;

/// A held JSON body, or 304 when the request already names its ETag.
pub(crate) fn cached_json(json: CachedJson, headers: &HeaderMap) -> Result<Response, ApiError> {
    let etag: Option<HeaderValue> = json
        .etag
        .map(|etag| HeaderValue::from_str(&etag))
        .transpose()
        .map_err(|error| ControlError::Invalid(format!("held response ETag: {error}")))?;
    let unchanged = etag.as_ref().is_some_and(|etag| {
        headers
            .get_all(header::IF_NONE_MATCH)
            .iter()
            .flat_map(|value| value.as_bytes().split(|byte| *byte == b','))
            .map(weak_stripped)
            .any(|tag| tag == ANY_ETAG.as_bytes() || etag.as_bytes() == tag)
    });
    let mut response = if unchanged {
        StatusCode::NOT_MODIFIED.into_response()
    } else {
        (
            [(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            )],
            json.body,
        )
            .into_response()
    };
    if let Some(etag) = etag {
        response.headers_mut().insert(header::ETAG, etag);
    }
    Ok(response)
}

/// One listed entity tag without its surrounding spaces or weak `W/` marks; issued tags are ASCII.
fn weak_stripped(mut tag: &[u8]) -> &[u8] {
    tag = tag.trim_ascii();
    while let Some(rest) = tag.strip_prefix(b"W/") {
        tag = rest;
    }
    tag
}
