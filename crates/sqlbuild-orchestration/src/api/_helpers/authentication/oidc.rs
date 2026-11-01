//! Browser-bound authorization-code/PKCE transport authenticates explicitly bound people only.

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::{Extension, Json, Router, middleware};
use openidconnect::core::CoreAuthenticationFlow;
use openidconnect::{CsrfToken, Nonce, PkceCodeChallenge};

use crate::api::_helpers::authentication::oidc_provider;
use crate::api::_helpers::authentication::password_transport::{
    browser_cookie, deny, guard, set_cookie,
};
use crate::api::constants::OIDC_FLOW_COOKIE;
use crate::api::errors::ApiError;
use crate::api::models::{
    AuthenticationDenial, BrowserOrigin, ExternalIdentityResponse, OidcAuthentication,
    OidcCallback, OidcRuntime,
};
use crate::credentials::constants::OIDC_FLOW_SECONDS;
use crate::credentials::main::authenticate_bound_person::authenticate_bound_person;
use crate::credentials::main::reserve_authentication_attempt::reserve_authentication_attempt;
use crate::credentials::main::{
    claim_oidc_flow::claim_oidc_flow, create_oidc_flow::create_oidc_flow,
};
use crate::credentials::models::{
    AuditContext, AuthenticationAttempt, ExternalIdentityMechanism, OidcFlowStart,
};
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

pub(crate) async fn router(
    store: &ControlStore,
    settings: Option<OidcAuthentication>,
) -> Result<Router<ControlStore>, ControlError> {
    let Some(settings) = settings else {
        return Ok(Router::new());
    };
    let runtime = oidc_provider::configure(settings)?;
    oidc_provider::client(&runtime).await?;
    Ok(Router::new()
        .route("/api/auth/oidc/login", get(login))
        .route("/api/auth/oidc/callback", get(callback))
        .layer(middleware::from_fn_with_state(store.clone(), guard))
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
        let mut response = deny(&store, &context, "oidc_login_throttled").await?;
        *response.status_mut() = StatusCode::TOO_MANY_REQUESTS;
        return Ok(response);
    }
    let client = match oidc_provider::client(&runtime).await {
        Ok(client) => client,
        Err(_) => return deny(&store, &context, "oidc_provider_unavailable").await,
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
    description = "Validates one browser-bound code and returns the explicitly bound person only; no session, bearer, provisioning or grants.",
    responses((status = 200, body = ExternalIdentityResponse), (status = 401, body = AuthenticationDenial)))]
pub(crate) async fn callback(
    State(store): State<ControlStore>,
    Extension(runtime): Extension<Arc<OidcRuntime>>,
    Extension(context): Extension<AuditContext>,
    (headers, Query(request)): (HeaderMap, Query<OidcCallback>),
) -> Result<Response, ApiError> {
    let Some(state) = request.state else {
        return deny(&store, &context, "missing_oidc_state").await;
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
        return deny(&store, &context, "invalid_oidc_browser_or_state").await;
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
                if authenticate_bound_person(
                    &store,
                    &context,
                    principal,
                    ExternalIdentityMechanism::Oidc,
                )
                .await?
                .is_some()
                {
                    Json(ExternalIdentityResponse {
                        principal_id: principal,
                    })
                    .into_response()
                } else {
                    deny(&store, &context, "inactive_oidc_person").await?
                }
            }
            None => deny(&store, &context, "unbound_oidc_subject").await?,
        },
        Err(_) => deny(&store, &context, "invalid_oidc_identity").await?,
    };
    set_cookie(&mut response, (OIDC_FLOW_COOKIE, ""), 0, (true, "Lax"))?;
    Ok(response)
}
