//! Serve the embedded web UI for every GET outside the API, falling back to its index for client routes.

use axum::Router;
use axum::http::{HeaderValue, Method, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};

use crate::api::constants::{
    API_PATH, API_PREFIX, AUTH_PREFIX, ROOT_PATH, UI_DOT_SEGMENTS, UI_DOUBLE_SLASH,
    UI_FRAME_POLICY, UI_IMMUTABLE_PREFIX, UI_INDEX_PATH, UI_REFUSED_ENCODINGS,
};
use crate::api::errors::ApiError;
use crate::api::models::UiAsset;

#[cfg(feature = "ui")]
include!(concat!(env!("OUT_DIR"), "/ui_assets.rs"));
#[cfg(not(feature = "ui"))]
pub(crate) static UI_ASSETS: &[UiAsset] = &[];

/// Every path no route takes: an API path is a JSON 404, never the UI's page; anything else is a UI file or the UI's page.
pub(crate) fn serve_ui(application: Router, assets: &'static [UiAsset]) -> Router {
    application.fallback(move |method: Method, uri: Uri| async move {
        respond(assets, &method, uri.path())
    })
}

fn respond(assets: &'static [UiAsset], method: &Method, path: &str) -> Response {
    if path == API_PATH || path.starts_with(API_PREFIX) {
        return ApiError {
            status: StatusCode::NOT_FOUND,
            message: "no such API endpoint".into(),
            decision_id: None,
            cause: None,
        }
        .into_response();
    }
    ui_response(assets, method, path)
}

/// An embedded UI file, or the UI's page for a client route; 404 outside the UI.
pub(crate) fn ui_response(assets: &'static [UiAsset], method: &Method, path: &str) -> Response {
    let outside_ui = path == API_PATH
        || path.starts_with(API_PREFIX)
        || path.starts_with(AUTH_PREFIX)
        || !canonical(path);
    if assets.is_empty() || outside_ui || (method != Method::GET && method != Method::HEAD) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let wanted = if path == ROOT_PATH {
        UI_INDEX_PATH
    } else {
        path
    };
    let file_like = wanted
        .rsplit('/')
        .next()
        .is_some_and(|name| name.contains('.'));
    let found = assets.iter().find(|asset| asset.path == wanted);
    let asset = match found {
        Some(asset) => asset,
        None if file_like => return StatusCode::NOT_FOUND.into_response(),
        None => match assets.iter().find(|asset| asset.path == UI_INDEX_PATH) {
            Some(index) => index,
            None => return StatusCode::NOT_FOUND.into_response(),
        },
    };
    let caching = if asset.path.starts_with(UI_IMMUTABLE_PREFIX) {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    let mut response = asset.body.into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(content_type(asset.path)),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(caching));
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(UI_FRAME_POLICY),
    );
    response
}

/// Only plain, already-normalised paths can name an embedded file or the shell.
fn canonical(path: &str) -> bool {
    let lowered = path.to_ascii_lowercase();
    !path.contains('\\')
        && !path.contains(UI_DOUBLE_SLASH)
        && !UI_REFUSED_ENCODINGS
            .iter()
            .any(|encoded| lowered.contains(encoded))
        && path
            .split('/')
            .all(|segment| !UI_DOT_SEGMENTS.contains(&segment))
}

fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or_default() {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "txt" => "text/plain; charset=utf-8",
        "png" => "image/png",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}
