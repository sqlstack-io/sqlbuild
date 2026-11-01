//! Password endpoints return an HttpOnly cookie, never a browser-readable session bearer.

use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router, middleware};

use crate::api::_helpers::authentication::password_transport::{authenticate, deny, guard};
use crate::api::constants::{
    HTTPS_SCHEME, PASSWORD_BODY_BYTES, PASSWORD_COOKIE, PASSWORD_CSRF_COOKIE, ROOT_PATH,
};
use crate::api::errors::ApiError;
use crate::api::models::{
    AuthenticationDenial, PasswordChangeRequest, PasswordLoginRequest, PasswordOrigin,
    PasswordSessionResponse,
};
use crate::credentials::constants::{LOGIN_WINDOW_SECONDS, PASSWORD_SESSION_SECONDS};
use crate::credentials::main::{
    login_password::login_password, reserve_password_attempt::reserve_password_attempt,
    revoke_session::revoke_session, set_password::set_password, verify_password::verify_password,
};
use crate::credentials::models::{
    AuditContext, PasswordAttempt, PasswordAuthority, PasswordLogin, SessionIdentity,
};
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

pub(crate) fn router(
    store: &ControlStore,
    origin: Option<String>,
) -> Result<Router<ControlStore>, ControlError> {
    let Some(origin) = origin else {
        return Ok(Router::new());
    };
    let url = reqwest::Url::parse(&origin).map_err(|_| {
        ControlError::Invalid("password authentication requires an HTTPS origin".into())
    })?;
    if url.scheme() != HTTPS_SCHEME
        || url.host().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != ROOT_PATH
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(ControlError::Invalid("password authentication requires an HTTPS origin without credentials, path, query or fragment".into()));
    }
    let protected = Router::new()
        .route("/api/auth/session", get(current))
        .route("/api/auth/logout", post(logout))
        .route("/api/auth/password", post(change_password))
        .route_layer(middleware::from_fn_with_state(store.clone(), authenticate));
    Ok(protected
        .route("/api/auth/password/login", post(login))
        .layer(DefaultBodyLimit::max(PASSWORD_BODY_BYTES))
        .layer(middleware::from_fn_with_state(store.clone(), guard))
        .layer(Extension(PasswordOrigin(
            url.origin().ascii_serialization(),
        ))))
}

#[utoipa::path(post, path = "/api/auth/password/login", operation_id = "loginPassword",
    request_body = PasswordLoginRequest,
    responses((status = 200, body = PasswordSessionResponse), (status = 401, body = AuthenticationDenial),
        (status = 429, body = AuthenticationDenial)))]
pub(crate) async fn login(
    State(store): State<ControlStore>,
    Extension(context): Extension<AuditContext>,
    Json(request): Json<PasswordLoginRequest>,
) -> Result<Response, ApiError> {
    match login_password(&store, &context, (&request.username, &request.password)).await? {
        PasswordLogin::Authenticated(session) => {
            let mut response = Json(PasswordSessionResponse {
                identity: session.identity,
                csrf_token: session.csrf.clone(),
            })
            .into_response();
            set_cookie(
                &mut response,
                (PASSWORD_COOKIE, &session.bearer),
                PASSWORD_SESSION_SECONDS,
                true,
            )?;
            set_cookie(
                &mut response,
                (PASSWORD_CSRF_COOKIE, &session.csrf),
                PASSWORD_SESSION_SECONDS,
                false,
            )?;
            Ok(response)
        }
        PasswordLogin::Denied {
            decision_id,
            throttled,
        } => {
            let mut response = (
                if throttled {
                    StatusCode::TOO_MANY_REQUESTS
                } else {
                    StatusCode::UNAUTHORIZED
                },
                Json(AuthenticationDenial {
                    error: "authentication rejected".into(),
                    decision_id,
                }),
            )
                .into_response();
            if throttled {
                response.headers_mut().insert(
                    header::RETRY_AFTER,
                    HeaderValue::from_str(&LOGIN_WINDOW_SECONDS.to_string()).map_err(|_| {
                        ControlError::Invalid("invalid login retry interval".into())
                    })?,
                );
            }
            Ok(response)
        }
    }
}

#[utoipa::path(get, path = "/api/auth/session", operation_id = "currentSession",
    responses((status = 200, body = SessionIdentity), (status = 401, body = AuthenticationDenial)),
    security(("session" = [])))]
pub(crate) async fn current(
    Extension(identity): Extension<SessionIdentity>,
) -> Json<SessionIdentity> {
    Json(identity)
}

#[utoipa::path(post, path = "/api/auth/logout", operation_id = "logoutSession",
    params(("x-sqb-csrf" = String, Header, description = "Session CSRF token")),
    responses((status = 204), (status = 401, body = AuthenticationDenial)),
    security(("session" = [])))]
pub(crate) async fn logout(
    State(store): State<ControlStore>,
    Extension(context): Extension<AuditContext>,
    Extension(identity): Extension<SessionIdentity>,
) -> Result<Response, ApiError> {
    revoke_session(&store, &context, &identity).await?;
    clear_cookies()
}

#[utoipa::path(post, path = "/api/auth/password", operation_id = "changePassword",
    params(("x-sqb-csrf" = String, Header, description = "Session CSRF token")),
    request_body = PasswordChangeRequest,
    responses((status = 204), (status = 401, body = AuthenticationDenial)),
    security(("session" = [])))]
pub(crate) async fn change_password(
    State(store): State<ControlStore>,
    Extension(context): Extension<AuditContext>,
    Extension(identity): Extension<SessionIdentity>,
    Json(request): Json<PasswordChangeRequest>,
) -> Result<Response, ApiError> {
    let (_, allowed) = reserve_password_attempt(
        &store,
        &context,
        PasswordAttempt::Reauthenticate(identity.principal_id),
    )
    .await?;
    if !allowed {
        return deny(&store, &context, "reauthentication_throttled").await;
    }
    let Some(proof) =
        verify_password(&store, identity.principal_id, &request.current_password).await?
    else {
        return deny(&store, &context, "invalid_current_password").await;
    };
    set_password(
        &store,
        &context,
        PasswordAuthority::Reauthenticated {
            proof,
            session_id: identity.session_id,
        },
        &request.new_password,
    )
    .await?;
    clear_cookies()
}

fn clear_cookies() -> Result<Response, ApiError> {
    let mut response = StatusCode::NO_CONTENT.into_response();
    set_cookie(&mut response, (PASSWORD_COOKIE, ""), 0, true)?;
    set_cookie(&mut response, (PASSWORD_CSRF_COOKIE, ""), 0, false)?;
    Ok(response)
}

fn set_cookie(
    response: &mut Response,
    (name, value): (&str, &str),
    max_age: i64,
    http_only: bool,
) -> Result<(), ApiError> {
    let attributes = if http_only { "; HttpOnly" } else { "" };
    let mut cookie = HeaderValue::from_str(&format!(
        "{name}={value}; Path=/; Secure; SameSite=Strict; Max-Age={max_age}{attributes}"
    ))
    .map_err(|_| ControlError::Invalid("could not encode a session cookie".into()))?;
    cookie.set_sensitive(true);
    response.headers_mut().append(header::SET_COOKIE, cookie);
    Ok(())
}
