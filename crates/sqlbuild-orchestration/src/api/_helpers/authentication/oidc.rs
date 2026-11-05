//! Browser-bound authorization-code/PKCE transport authenticates explicitly bound people only.

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::{Extension, Router, middleware};
use openidconnect::core::CoreAuthenticationFlow;
use openidconnect::{CsrfToken, Nonce, PkceCodeChallenge};

use crate::api::_helpers::authentication::password_transport::{
    browser_cookie, deny, external_session, guard, set_cookie,
};
use crate::api::_helpers::authentication::{oidc_provider, password_sessions};
use crate::api::constants::OIDC_FLOW_COOKIE;
use crate::api::errors::ApiError;
use crate::api::models::{
    AuthenticationDenial, BrowserLoginMethod, BrowserOrigin, ExternalIdentityResponse,
    OidcAuthentication, OidcCallback, OidcRuntime,
};
use crate::credentials::constants::OIDC_FLOW_SECONDS;
use crate::credentials::main::audit::reserve_authentication_attempt::reserve_authentication_attempt;
use crate::credentials::main::sessions::authenticate_bound_person::authenticate_bound_person;
use crate::credentials::main::sessions::{
    claim_oidc_flow::claim_oidc_flow, create_oidc_flow::create_oidc_flow,
};
use crate::credentials::models::{AuditContext, AuthenticationAttempt, OidcFlowStart};
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

pub(crate) async fn router(
    store: &ControlStore,
    settings: OidcAuthentication,
) -> Result<Router<ControlStore>, ControlError> {
    let runtime = oidc_provider::configure(settings)?;
    oidc_provider::client(&runtime).await?;
    Ok(Router::new()
        .route("/api/auth/oidc/login", get(login))
        .route("/api/auth/oidc/callback", get(callback))
        .merge(password_sessions::protected(store))
        .merge(super::cli_login::router(BrowserLoginMethod::Oidc))
        .layer(middleware::from_fn(guard))
        .layer(Extension(BrowserOrigin(runtime.settings.origin.clone())))
        .layer(Extension(Arc::new(runtime))))
}

#[utoipa::path(get, path = "/api/auth/oidc/login", operation_id = "loginOidc",
    description = "Begins a browser-bound code flow with S256 PKCE; no account provisioning or grants.",
    responses((status = 303), (status = 401, body = AuthenticationDenial), (status = 429, body = AuthenticationDenial)))]
pub(crate) async fn login(
    State(store): State<ControlStore>,
    Extension(runtime): Extension<Arc<OidcRuntime>>,
    Extension(context): Extension<AuditContext>,
) -> Result<Response, ApiError> {
    let (_, allowed) =
        reserve_authentication_attempt(&store, &context, AuthenticationAttempt::IdentityProvider)
            .await?;
    if !allowed {
        let mut response = deny(&context)?;
        *response.status_mut() = StatusCode::TOO_MANY_REQUESTS;
        return Ok(response);
    }
    let client = match oidc_provider::client(&runtime).await {
        Ok(client) => client,
        Err(_) => return deny(&context),
    };
    let (challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    let (url, state, nonce) = client
        .authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        )
        .set_pkce_challenge(challenge)
        .url();
    let flow = OidcFlowStart {
        state,
        nonce,
        pkce_verifier,
        browser: CsrfToken::new_random(),
    };
    create_oidc_flow(&store, &context, &runtime.configuration_id, &flow).await?;
    let mut response = Redirect::to(url.as_str()).into_response();
    set_cookie(
        &mut response,
        (OIDC_FLOW_COOKIE, flow.browser.secret()),
        OIDC_FLOW_SECONDS,
        (true, "Lax"),
    )?;
    Ok(response)
}

#[utoipa::path(get, path = "/api/auth/oidc/callback", operation_id = "oidcIdentity",
    description = "Validates one browser-bound code and issues a bounded browser session for an explicitly bound person; no provisioning, personal API tokens or implicit grants.",
    responses((status = 200, body = ExternalIdentityResponse), (status = 401, body = AuthenticationDenial)))]
pub(crate) async fn callback(
    State(store): State<ControlStore>,
    Extension(runtime): Extension<Arc<OidcRuntime>>,
    Extension(context): Extension<AuditContext>,
    (headers, Query(request)): (HeaderMap, Query<OidcCallback>),
) -> Result<Response, ApiError> {
    let Some(state) = request.state else {
        return deny(&context);
    };
    let Some(flow) = claim_oidc_flow(
        &store,
        &context,
        &runtime.configuration_id,
        (
            state.secret(),
            browser_cookie(&headers, OIDC_FLOW_COOKIE).unwrap_or_default(),
        ),
    )
    .await?
    else {
        return deny(&context);
    };
    let subject = match request.code {
        Some(code) => oidc_provider::verify_code(&runtime, code, flow).await,
        None => Err(ControlError::Invalid(
            "OIDC authorization was not completed".into(),
        )),
    };
    let mut response = match subject {
        Ok(subject) => match runtime.settings.identities.get(&subject).copied() {
            Some(principal) => {
                if let Some(session) =
                    authenticate_bound_person(&store, &context, principal).await?
                {
                    external_session(session)?
                } else {
                    deny(&context)?
                }
            }
            None => deny(&context)?,
        },
        Err(_) => deny(&context)?,
    };
    if browser_cookie(&headers, "__Host-sqb-cli-return") == Some("1") {
        set_cookie(
            &mut response,
            ("__Host-sqb-cli-return", ""),
            0,
            (false, "Lax"),
        )?;
        if response.status().is_success() {
            *response.status_mut() = StatusCode::SEE_OTHER;
            response.headers_mut().insert(
                header::LOCATION,
                header::HeaderValue::from_static("/auth/cli"),
            );
            *response.body_mut() = axum::body::Body::empty();
        }
    }
    set_cookie(&mut response, (OIDC_FLOW_COOKIE, ""), 0, (true, "Lax"))?;
    Ok(response)
}
