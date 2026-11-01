//! Browser credentials use host-only cookies; peer identity never comes from forwarded headers.

use std::net::SocketAddr;

use axum::extract::{ConnectInfo, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde_json::json;
use uuid::Uuid;

use crate::api::constants::{PASSWORD_COOKIE, PASSWORD_CSRF_HEADER};
use crate::api::errors::ApiError;
use crate::api::models::{AuthenticationDenial, PasswordOrigin};
use crate::credentials::main::authenticate_session::authenticate_session;
use crate::credentials::main::record_authentication_denial::record_authentication_denial;
use crate::credentials::models::{AuditContext, SessionAccess};
use crate::store::models::ControlStore;

pub(crate) async fn guard(
    State(store): State<ControlStore>,
    Extension(origin): Extension<PasswordOrigin>,
    mut request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .ok_or_else(|| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "trusted request peer is unavailable".into(),
        })?
        .0;
    let context = AuditContext {
        actor: "anonymous".into(),
        granting_role: "authentication".into(),
        request_id: Uuid::now_v7(),
        ip_address: Some(peer.ip().to_canonical()),
    };
    let valid_host = single_header(request.headers(), header::HOST.as_str())
        == origin.0.strip_prefix("https://");
    let valid_origin = match single_header(request.headers(), header::ORIGIN.as_str()) {
        Some(value) => value == origin.0,
        None => request.method().is_safe() && !request.headers().contains_key(header::ORIGIN),
    };
    let mut response = if !valid_host || !valid_origin {
        deny(&store, &context, "invalid_origin").await?
    } else {
        request.extensions_mut().insert(context);
        next.run(request).await
    };
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    Ok(response)
}

pub(crate) async fn authenticate(
    State(store): State<ControlStore>,
    mut request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let context = request
        .extensions()
        .get::<AuditContext>()
        .cloned()
        .ok_or_else(|| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "trusted authentication context is unavailable".into(),
        })?;
    let access = if request.method().is_safe() {
        SessionAccess::Read
    } else {
        SessionAccess::Write {
            csrf: single_header(request.headers(), PASSWORD_CSRF_HEADER).unwrap_or_default(),
        }
    };
    let identity = authenticate_session(
        &store,
        session_cookie(request.headers()).unwrap_or_default(),
        access,
    )
    .await?;
    let Some(identity) = identity else {
        return deny(&store, &context, "invalid_session_or_csrf").await;
    };
    let mut context = context;
    context.actor = format!("principal:{}", identity.principal_id);
    context.granting_role = "password-session".into();
    request.extensions_mut().insert(context);
    request.extensions_mut().insert(identity);
    Ok(next.run(request).await)
}

pub(crate) fn single_header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    let mut values = headers.get_all(name).iter();
    let Ok(value) = values.next()?.to_str() else {
        return None;
    };
    values.next().is_none().then_some(value)
}

fn session_cookie(headers: &HeaderMap) -> Option<&str> {
    let mut session = None;
    for header in headers.get_all(header::COOKIE) {
        let Ok(value) = header.to_str() else {
            return None;
        };
        for cookie in value.split(';') {
            let Some((name, value)) = cookie.trim().split_once('=') else {
                continue;
            };
            if name == PASSWORD_COOKIE {
                if session.is_some() {
                    return None;
                }
                session = Some(value);
            }
        }
    }
    session
}

pub(crate) async fn deny(
    store: &ControlStore,
    context: &AuditContext,
    reason: &str,
) -> Result<Response, ApiError> {
    let decision_id = record_authentication_denial(
        store,
        context,
        (context.request_id, reason),
        json!({"kind": "browser_authentication"}),
    )
    .await?;
    Ok((
        StatusCode::UNAUTHORIZED,
        Json(AuthenticationDenial {
            error: "authentication rejected".into(),
            decision_id,
        }),
    )
        .into_response())
}
