//! Password endpoints return an HttpOnly cookie, never a browser-readable session bearer.

use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router, middleware};

use crate::api::_helpers::authentication::password_transport::{
    authenticate, deny, guard, parse_origin, set_cookie,
};
use crate::api::constants::{PASSWORD_BODY_BYTES, PASSWORD_COOKIE, PASSWORD_CSRF_COOKIE};
use crate::api::errors::ApiError;
use crate::api::models::{
    AuthenticationDenial, PasswordChangeRequest, PasswordLoginRequest, PasswordSessionResponse,
};
use crate::credentials::constants::{LOGIN_WINDOW_SECONDS, PASSWORD_SESSION_SECONDS};
use crate::credentials::main::{
    login_password::login_password, reserve_authentication_attempt::reserve_authentication_attempt,
    revoke_session::revoke_session, set_password::set_password, verify_password::verify_password,
};
use crate::credentials::models::{
    AuditContext, AuthenticationAttempt, PasswordAuthority, PasswordLogin, SessionIdentity,
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
    let origin = parse_origin(&origin, "password")?;
    let protected = Router::new()
        .route("/api/auth/session", get(current))
        .route("/api/auth/logout", post(logout))
        .route("/api/auth/password", post(change_password))
        .route(
            "/api/admin/people/{principal_id}",
            post(crate::api::_helpers::resources::administrators::change),
        )
        .route(
            "/api/admin/roles",
            post(crate::api::_helpers::resources::administrators::assign),
        )
        .route(
            "/api/control/runs",
            post(crate::api::_helpers::resources::person_operations::request_job),
        )
        .route(
            "/api/control/runs/cancel",
            post(crate::api::_helpers::resources::person_operations::cancel_run),
        )
        .route(
            "/api/control/runs/rerun",
            post(crate::api::_helpers::resources::person_operations::rerun),
        )
        .route(
            "/api/control/triggers",
            post(crate::api::_helpers::resources::person_operations::change_trigger),
        )
        .route(
            "/api/control/locations/{location_id}/targets/{target}/runs",
            get(crate::api::_helpers::resources::person_operations::recent_runs),
        )
        .route_layer(middleware::from_fn_with_state(store.clone(), authenticate));
    Ok(protected
        .route("/api/auth/password/login", post(login))
        .layer(DefaultBodyLimit::max(PASSWORD_BODY_BYTES))
        .layer(middleware::from_fn_with_state(store.clone(), guard))
        .layer(Extension(origin)))
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
    let (_, allowed) = reserve_authentication_attempt(
        &store,
        &context,
        AuthenticationAttempt::Reauthenticate(identity.principal_id),
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
    set_cookie(&mut response, (PASSWORD_COOKIE, ""), 0, (true, "Strict"))?;
    set_cookie(
        &mut response,
        (PASSWORD_CSRF_COOKIE, ""),
        0,
        (false, "Strict"),
    )?;
    Ok(response)
}
