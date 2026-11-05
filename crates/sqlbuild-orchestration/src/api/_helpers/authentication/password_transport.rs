//! Browser credentials use host-only cookies; peer identity never comes from forwarded headers.

use std::net::SocketAddr;

use axum::extract::{ConnectInfo, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use uuid::Uuid;

use crate::api::constants::{PASSWORD_COOKIE, PASSWORD_CSRF_COOKIE, PASSWORD_CSRF_HEADER};
use crate::api::errors::ApiError;
use crate::api::models::{AuthenticationDenial, BrowserOrigin, ExternalIdentityResponse};
use crate::credentials::constants::PASSWORD_SESSION_SECONDS;
use crate::credentials::main::sessions::authenticate_session::authenticate_session;
use crate::credentials::models::{AuditContext, AuthenticatedPerson, IssuedSession, SessionAccess};
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

pub(crate) async fn guard(
    Extension(origin): Extension<BrowserOrigin>,
    mut request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .ok_or_else(|| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "trusted request peer is unavailable".into(),
            decision_id: None,
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
        deny(&context)?
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
            decision_id: None,
        })?;
    let access =
        if request.method().is_safe() || request.headers().contains_key(header::AUTHORIZATION) {
            SessionAccess::Read
        } else {
            SessionAccess::Write {
                csrf: single_header(request.headers(), PASSWORD_CSRF_HEADER).unwrap_or_default(),
            }
        };
    let identity =
        authenticate_session(&store, session_credentials(request.headers()).0, access).await?;
    let Some(identity) = identity else {
        return deny(&context);
    };
    let mut context = context;
    context.actor = format!("principal:{}", identity.principal_id);
    context.granting_role = "browser-session".into();
    request.extensions_mut().insert(context);
    request.extensions_mut().insert(AuthenticatedPerson(
        identity.principal_id,
        Some(identity.session_id),
    ));
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

pub(crate) fn session_credentials(headers: &HeaderMap) -> (&str, &str) {
    let cookie = browser_cookie(headers, PASSWORD_COOKIE);
    let authorization = if headers.contains_key(header::AUTHORIZATION) {
        if headers.contains_key(header::COOKIE) {
            return ("", "");
        }
        single_header(headers, header::AUTHORIZATION.as_str())
            .and_then(|value| value.strip_prefix("Session "))
    } else {
        cookie
    };
    (
        authorization.unwrap_or_default(),
        single_header(headers, PASSWORD_CSRF_HEADER).unwrap_or_default(),
    )
}

pub(crate) fn browser_cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    let mut session = None;
    for header in headers.get_all(header::COOKIE) {
        let Ok(value) = header.to_str() else {
            return None;
        };
        for cookie in value.split(';') {
            let Some((cookie_name, value)) = cookie.trim().split_once('=') else {
                continue;
            };
            if cookie_name == name {
                if session.is_some() {
                    return None;
                }
                session = Some(value);
            }
        }
    }
    session
}

pub(crate) fn external_session(session: IssuedSession) -> Result<Response, ApiError> {
    let response = Json(ExternalIdentityResponse {
        principal_id: session.identity.principal_id,
        session_id: session.identity.session_id,
        expires_at: session.identity.expires_at,
        csrf_token: session.csrf.clone(),
    })
    .into_response();
    session_cookies(response, &session)
}

pub(crate) fn session_cookies(
    mut response: Response,
    session: &IssuedSession,
) -> Result<Response, ApiError> {
    set_cookie(
        &mut response,
        (PASSWORD_COOKIE, &session.bearer),
        PASSWORD_SESSION_SECONDS,
        (true, "Strict"),
    )?;
    set_cookie(
        &mut response,
        (PASSWORD_CSRF_COOKIE, &session.csrf),
        PASSWORD_SESSION_SECONDS,
        (false, "Strict"),
    )?;
    Ok(response)
}

pub(crate) fn set_cookie(
    response: &mut Response,
    (name, value): (&str, &str),
    max_age: i64,
    (http_only, same_site): (bool, &str),
) -> Result<(), ApiError> {
    let attributes = if http_only { "; HttpOnly" } else { "" };
    let mut cookie = HeaderValue::from_str(&format!(
        "{name}={value}; Path=/; Secure; SameSite={same_site}; Max-Age={max_age}{attributes}"
    ))
    .map_err(|_| ControlError::Invalid("could not encode a session cookie".into()))?;
    cookie.set_sensitive(true);
    response.headers_mut().append(header::SET_COOKIE, cookie);
    Ok(())
}

/// Anonymous denials are not durably audited; the request ID identifies the decision.
pub(crate) fn deny(context: &AuditContext) -> Result<Response, ApiError> {
    let decision_id = context.request_id;
    Ok((
        StatusCode::UNAUTHORIZED,
        Json(AuthenticationDenial {
            error: "authentication rejected".into(),
            decision_id,
        }),
    )
        .into_response())
}
