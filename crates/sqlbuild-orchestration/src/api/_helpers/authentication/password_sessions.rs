//! Password endpoints return an HttpOnly cookie, never a browser-readable session bearer.

use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router, middleware};

use crate::api::_helpers::authentication::password_transport::{
    authenticate, deny, guard, session_cookies, set_cookie,
};
use crate::api::constants::{PASSWORD_BODY_BYTES, PASSWORD_COOKIE, PASSWORD_CSRF_COOKIE};
use crate::api::errors::ApiError;
use crate::api::main::parse_origin::parse_origin;
use crate::api::models::{
    AuthenticationDenial, BrowserLoginMethod, BrowserOrigin, PasswordChangeRequest,
    PasswordLoginRequest, PasswordSessionResponse,
};
use crate::credentials::constants::LOGIN_WINDOW_SECONDS;
use crate::credentials::main::{
    audit::reserve_authentication_attempt::reserve_authentication_attempt,
    sessions::login_password::login_password, sessions::revoke_session::revoke_session,
    sessions::set_password::set_password, sessions::verify_password::verify_password,
};
use crate::credentials::models::{
    AuditContext, AuthenticationAttempt, PasswordAuthority, PasswordLogin, SessionIdentity,
};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

/// Session-authenticated person routes, plus routes acting on the caller's own session.
pub(crate) fn protected(store: &ControlStore) -> Router<ControlStore> {
    person_routes()
        .route_layer(middleware::from_fn_with_state(store.clone(), authenticate))
        .merge(session_routes(store))
}

/// Routes that act on the caller's own session.
pub(crate) fn session_routes(store: &ControlStore) -> Router<ControlStore> {
    Router::new()
        .route("/api/auth/session", get(current))
        .route("/api/auth/logout", post(logout))
        .route_layer(middleware::from_fn_with_state(store.clone(), authenticate))
        .layer(DefaultBodyLimit::max(PASSWORD_BODY_BYTES))
}

/// Person operations; the caller layers the authentication that identifies the person.
pub(crate) fn person_routes() -> Router<ControlStore> {
    Router::new()
        .route("/api/auth/cli/approve", post(super::cli_login::approve))
        .route(
            "/api/control/bootstrap",
            get(crate::api::_helpers::resources::person_bootstrap::read),
        )
        .route(
            "/api/control/locations/{location_id}/targets/{target}/changes",
            get(crate::api::_helpers::resources::changes::subscribe_person),
        )
        .route(
            "/api/control/audit",
            get(crate::api::_helpers::resources::audit::read),
        )
        .route(
            "/api/admin/people/{principal_id}",
            post(crate::api::_helpers::resources::administrators::change),
        )
        .route(
            "/api/admin/people",
            get(crate::api::_helpers::resources::administrators::people),
        )
        .route(
            "/api/admin/locations/{location_id}/targets/{target}/roles",
            get(crate::api::_helpers::resources::administrators::definitions),
        )
        .route(
            "/api/admin/roles",
            post(crate::api::_helpers::resources::administrators::assign)
                .get(crate::api::_helpers::resources::administrators::holdings),
        )
        .route(
            "/api/control/runs",
            post(crate::api::_helpers::resources::person_operations::request_job),
        )
        .route(
            "/api/control/runs/adhoc",
            post(crate::api::_helpers::resources::person_operations::request_adhoc),
        )
        .route(
            "/api/control/runs/adhoc/check",
            post(crate::api::_helpers::resources::person_operations::check_adhoc),
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
            "/api/control/locations/{location_id}/targets/{target}/connection-tests",
            post(crate::api::_helpers::resources::person_location_work::request),
        )
        .route(
            "/api/control/locations/{location_id}/targets/{target}/connection-tests/{request_id}",
            get(crate::api::_helpers::resources::person_location_work::result),
        )
        .route(
            "/api/control/locations/{location_id}/targets/{target}/runs",
            get(crate::api::_helpers::resources::person_operations::recent_runs),
        )
        .route(
            "/api/control/locations/{location_id}/targets/{target}/runs/{run_id}/nodes",
            get(crate::api::_helpers::resources::person_reads::run_nodes),
        )
        .route(
            "/api/control/locations/{location_id}/targets/{target}/nodes",
            get(crate::api::_helpers::resources::person_reads::nodes),
        )
        .route(
            "/api/control/locations/{location_id}/targets/{target}/jobs",
            get(crate::api::_helpers::resources::person_reads::jobs),
        )
        .route(
            "/api/control/locations/{location_id}/targets/{target}/triggers",
            get(crate::api::_helpers::resources::person_reads::triggers),
        )
        .route(
            "/api/control/locations/{location_id}/targets/{target}/triggers/{trigger_id}/ticks",
            get(crate::api::_helpers::resources::person_reads::ticks),
        )
        .route(
            "/api/control/locations/{location_id}/targets/{target}/snapshots",
            get(crate::api::_helpers::resources::person_reads::snapshots),
        )
        .route(
            "/api/control/locations/{location_id}/targets/{target}/health",
            get(crate::api::_helpers::resources::person_reads::health),
        )
        .route(
            "/api/control/platform/problems",
            get(crate::api::_helpers::resources::person_reads::problems),
        )
        .route(
            "/api/control/locations/{location_id}/targets/{target}/catalog",
            get(crate::api::_helpers::resources::person_operations::catalog),
        )
        .route(
            "/api/control/locations/{location_id}/targets/{target}/statistics",
            get(crate::api::_helpers::resources::person_operations::statistics),
        )
        .route(
            "/api/control/locations/{location_id}/targets/{target}/fingerprint",
            get(crate::api::_helpers::resources::person_operations::fingerprint),
        )
        .route(
            "/api/control/locations/{location_id}/targets/{target}/node-status",
            get(crate::api::_helpers::resources::person_operations::node_status),
        )
        .route(
            "/api/control/locations/{location_id}/targets/{target}/nodes/{node_key}/sql",
            get(crate::api::_helpers::resources::person_node_sql::read),
        )
        .route(
            "/api/control/locations/{location_id}/targets/{target}/checks",
            get(crate::api::_helpers::resources::person_checks::list),
        )
        .route(
            "/api/control/locations/{location_id}/targets/{target}/checks/{binding_key}",
            get(crate::api::_helpers::resources::person_checks::history),
        )
        .route(
            "/api/control/locations/{location_id}/targets/{target}/checks/{binding_key}/sql",
            get(crate::api::_helpers::resources::person_checks::sql),
        )
        .route(
            "/api/control/locations/{location_id}/targets/{target}/checks/{binding_key}/results/{event_id}/evidence",
            get(crate::api::_helpers::resources::person_checks::evidence),
        )
        .route(
            "/api/control/locations/{location_id}/targets/{target}/maintenance",
            get(crate::api::_helpers::resources::person_maintenance::read),
        )
        .route(
            "/api/control/locations/{location_id}/targets/{target}/microbatches/{model}",
            get(crate::api::_helpers::resources::person_maintenance::coverage),
        )
        .layer(DefaultBodyLimit::max(PASSWORD_BODY_BYTES))
}

pub(crate) fn router(
    store: &ControlStore,
    origin: String,
) -> Result<Router<ControlStore>, ControlError> {
    let origin = BrowserOrigin(parse_origin(&origin, "password")?);
    let protected = protected(store).merge(
        Router::new()
            .route("/api/auth/password", post(change_password))
            .route_layer(middleware::from_fn_with_state(store.clone(), authenticate)),
    );
    Ok(protected
        .route("/api/auth/password/login", post(login))
        .merge(super::cli_login::router(BrowserLoginMethod::Password))
        .layer(DefaultBodyLimit::max(PASSWORD_BODY_BYTES))
        .layer(middleware::from_fn(guard))
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
            let response = Json(PasswordSessionResponse {
                identity: session.identity.clone(),
                csrf_token: session.csrf.clone(),
            })
            .into_response();
            session_cookies(response, &session)
        }
        PasswordLogin::Denied { throttled } => {
            let mut response = (
                if throttled {
                    StatusCode::TOO_MANY_REQUESTS
                } else {
                    StatusCode::UNAUTHORIZED
                },
                Json(AuthenticationDenial {
                    error: "authentication rejected".into(),
                    request_id: context.request_id,
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
    security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn current(
    Extension(identity): Extension<SessionIdentity>,
) -> Json<SessionIdentity> {
    Json(identity)
}

#[utoipa::path(post, path = "/api/auth/logout", operation_id = "logoutSession",
    params(("x-sqb-csrf" = String, Header, description = "Session CSRF token")),
    responses((status = 204), (status = 401, body = AuthenticationDenial)),
    security(("session" = []), ("cliSession" = [])))]
pub(crate) async fn logout(
    State(store): State<ControlStore>,
    Extension(context): Extension<AuditContext>,
    Extension(identity): Extension<SessionIdentity>,
) -> Result<Response, ApiError> {
    let (transaction, _) = revoke_session(
        begin(&store, TransactionMode::Write).await?,
        &context,
        (identity.principal_id, Some(identity.session_id)),
    )
    .await?;
    commit(transaction).await?;
    clear_cookies()
}

#[utoipa::path(post, path = "/api/auth/password", operation_id = "changePassword",
    params(("x-sqb-csrf" = String, Header, description = "Session CSRF token")),
    request_body = PasswordChangeRequest,
    responses((status = 204), (status = 401, body = AuthenticationDenial)),
    security(("session" = []), ("cliSession" = [])))]
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
        return deny(&context);
    }
    let Some(proof) =
        verify_password(&store, identity.principal_id, &request.current_password).await?
    else {
        return deny(&context);
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
