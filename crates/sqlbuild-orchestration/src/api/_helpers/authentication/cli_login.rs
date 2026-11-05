//! Only the exchange secret can receive a CLI credential; a browser person approves it.

use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};

use crate::api::_helpers::authentication::password_transport::deny;
use crate::api::constants::PASSWORD_BODY_BYTES;
use crate::api::errors::ApiError;
use crate::api::models::{
    AuthenticationDenial, BrowserLoginMethod, CliLoginApprovalRequest, CliLoginClaimRequest,
    CliLoginSessionResponse, CliLoginStartRequest,
};
use crate::credentials::main::audit::reserve_authentication_attempt::reserve_authentication_attempt;
use crate::credentials::main::cli_login::{
    approve_login::approve_login, claim_login::claim_login, start_login::start_login,
};
use crate::credentials::models::{
    AuditContext, AuthenticatedPerson, AuthenticationAttempt, ClaimedLogin, StartedLogin,
};
use crate::store::models::ControlStore;

pub(crate) fn router(method: BrowserLoginMethod) -> Router<ControlStore> {
    Router::new()
        .route("/auth/cli", get(page))
        .route("/auth/cli.js", get(script))
        .route("/api/auth/cli/start", post(start))
        .route("/api/auth/cli/claim", post(claim))
        .layer(Extension(method))
        .layer(DefaultBodyLimit::max(PASSWORD_BODY_BYTES))
}

async fn page(Extension(method): Extension<BrowserLoginMethod>) -> Response {
    let method = match method {
        BrowserLoginMethod::Password => "password",
        BrowserLoginMethod::Oidc => "oidc",
        BrowserLoginMethod::Proxy => "proxy",
    };
    let mut response =
        Html(include_str!("cli_login.html").replace("__LOGIN_METHOD__", method)).into_response();
    response.headers_mut().insert(header::CONTENT_SECURITY_POLICY,
        header::HeaderValue::from_static("default-src 'none'; script-src 'self'; style-src 'unsafe-inline'; connect-src 'self'; frame-ancestors 'none'; form-action 'self'; base-uri 'none'"));
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        header::HeaderValue::from_static("no-referrer"),
    );
    response
}

async fn script() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("cli_login.js"),
    )
}

#[utoipa::path(post, path = "/api/auth/cli/start", operation_id = "startCliLogin",
    request_body = CliLoginStartRequest, responses((status = 200, body = StartedLogin),
    (status = 429, body = AuthenticationDenial)))]
pub(crate) async fn start(
    State(store): State<ControlStore>,
    Extension(context): Extension<AuditContext>,
    Json(request): Json<CliLoginStartRequest>,
) -> Result<Response, ApiError> {
    let (_, allowed) =
        reserve_authentication_attempt(&store, &context, AuthenticationAttempt::IdentityProvider)
            .await?;
    if !allowed {
        let mut response = deny(&context)?;
        *response.status_mut() = StatusCode::TOO_MANY_REQUESTS;
        return Ok(response);
    }
    Ok(Json(start_login(&store, &context, &request.client_hash).await?).into_response())
}

#[utoipa::path(post, path = "/api/auth/cli/approve", operation_id = "approveCliLogin",
    request_body = CliLoginApprovalRequest, security(("session" = []), ("proxy" = [])),
    responses((status = 204), (status = 401, body = AuthenticationDenial)))]
pub(crate) async fn approve(
    State(store): State<ControlStore>,
    (Extension(context), Extension(person)): (
        Extension<AuditContext>,
        Extension<AuthenticatedPerson>,
    ),
    headers: HeaderMap,
    Json(request): Json<CliLoginApprovalRequest>,
) -> Result<Response, ApiError> {
    if !headers.contains_key(header::AUTHORIZATION)
        && approve_login(
            &store,
            &context,
            person,
            (&request.browser_code, request.approve),
        )
        .await?
    {
        Ok(StatusCode::NO_CONTENT.into_response())
    } else {
        deny(&context)
    }
}

#[utoipa::path(post, path = "/api/auth/cli/claim", operation_id = "claimCliLogin",
    request_body = CliLoginClaimRequest, responses((status = 200, body = CliLoginSessionResponse),
    (status = 202), (status = 401, body = AuthenticationDenial)))]
pub(crate) async fn claim(
    State(store): State<ControlStore>,
    Extension(context): Extension<AuditContext>,
    Json(request): Json<CliLoginClaimRequest>,
) -> Result<Response, ApiError> {
    match claim_login(&store, &context, (request.flow_id, &request.client_secret)).await? {
        ClaimedLogin::Pending => Ok(StatusCode::ACCEPTED.into_response()),
        ClaimedLogin::Unavailable => deny(&context),
        ClaimedLogin::Authenticated(session) => Ok(Json(CliLoginSessionResponse {
            identity: session.identity,
            bearer: session.bearer,
            csrf_token: session.csrf,
        })
        .into_response()),
    }
}
