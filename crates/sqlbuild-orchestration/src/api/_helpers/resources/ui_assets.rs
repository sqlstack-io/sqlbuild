//! Serve the embedded web UI for every GET outside the API, falling back to its index for client routes.

use axum::Router;
use axum::http::{HeaderValue, Method, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};

use crate::api::constants::{
    API_PATH, API_PREFIX, AUTH_PREFIX, ROOT_PATH, UI_FRAME_POLICY, UI_IMMUTABLE_PREFIX,
    UI_INDEX_PATH,
};
use crate::api::models::UiAsset;

#[cfg(feature = "ui")]
include!(concat!(env!("OUT_DIR"), "/ui_assets.rs"));
#[cfg(not(feature = "ui"))]
pub(crate) static UI_ASSETS: &[UiAsset] = &[];

pub(crate) fn serve_ui(application: Router, assets: &'static [UiAsset]) -> Router {
    if assets.is_empty() {
        return application;
    }
    application.fallback(move |method: Method, uri: Uri| async move {
        respond(assets, &method, uri.path())
    })
}

fn respond(assets: &'static [UiAsset], method: &Method, path: &str) -> Response {
    let outside_ui =
        path == API_PATH || path.starts_with(API_PREFIX) || path.starts_with(AUTH_PREFIX);
    if outside_ui || (method != Method::GET && method != Method::HEAD) {
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
