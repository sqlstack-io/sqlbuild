//! Proxy subjects resolve only through explicit server-owned bindings and the real socket peer.

use std::sync::Arc;

use crate::api::_helpers::authentication::password_sessions;
use crate::api::_helpers::authentication::password_transport::{
    deny, external_session, guard, single_header,
};
use crate::api::constants::PROXY_USER_HEADER;
use crate::api::errors::ApiError;
use crate::api::main::parse_origin::parse_origin;
use crate::api::models::{
    AuthenticationDenial, BrowserLoginMethod, BrowserOrigin, ExternalIdentityResponse,
    TrustedProxyAuthentication,
};
use crate::credentials::main::audit::reserve_authentication_attempt::reserve_authentication_attempt;
use crate::credentials::main::sessions::authenticate_bound_person::authenticate_bound_person;
use crate::credentials::models::{AuditContext, AuthenticationAttempt, ExternalIdentityMechanism};
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::Response;
use axum::routing::get;
use axum::{Extension, Router, middleware};

pub(crate) fn router(
    store: &ControlStore,
    settings: Option<TrustedProxyAuthentication>,
) -> Result<Router<ControlStore>, ControlError> {
    let Some(mut settings) = settings else {
        return Ok(Router::new());
    };
    let origin = BrowserOrigin(parse_origin(&settings.origin, "trusted-proxy")?);
    if settings.trusted_peers.is_empty() {
        return Err(ControlError::Invalid(
            "trusted-proxy authentication requires explicit trusted peer IP addresses".into(),
        ));
    }
    if settings.identities.keys().any(|subject| {
        subject.trim().is_empty()
            || subject.trim() != subject
            || match HeaderValue::from_str(subject) {
                Ok(value) => value.to_str().is_err(),
                Err(_) => true,
            }
    }) {
        return Err(ControlError::Invalid(
            "trusted-proxy subjects must be nonempty, unpadded ASCII header values".into(),
        ));
    }
    settings.trusted_peers = settings
        .trusted_peers
        .into_iter()
        .map(|peer| peer.to_canonical())
        .collect();
    Ok(Router::new()
        .route("/api/auth/proxy", get(identity))
        .merge(password_sessions::protected(store))
        .merge(super::cli_login::router(BrowserLoginMethod::Proxy))
        .layer(middleware::from_fn_with_state(store.clone(), guard))
        .layer(Extension(origin))
        .layer(Extension(Arc::new(settings))))
}

#[utoipa::path(get, path = "/api/auth/proxy", operation_id = "proxyIdentity",
    description = "Requires a configured socket peer and upstream-replaced X-SQB-User subject; issues a bounded browser session without provisioning, personal API tokens or implicit grants.",
    security(("proxy" = [])),
    responses((status = 200, body = ExternalIdentityResponse), (status = 401, body = AuthenticationDenial), (status = 429, body = AuthenticationDenial)))]
pub(crate) async fn identity(
    State(store): State<ControlStore>,
    Extension(settings): Extension<Arc<TrustedProxyAuthentication>>,
    Extension(context): Extension<AuditContext>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    if !context
        .ip_address
        .is_some_and(|peer| settings.trusted_peers.contains(&peer))
    {
        return deny(&store, &context, "untrusted_proxy_peer").await;
    }
    let principal = single_header(&headers, PROXY_USER_HEADER)
        .and_then(|subject| settings.identities.get(subject))
        .copied();
    let Some(principal_id) = principal else {
        return deny(&store, &context, "unbound_proxy_subject").await;
    };
    let (_, allowed) =
        reserve_authentication_attempt(&store, &context, AuthenticationAttempt::IdentityProvider)
            .await?;
    if !allowed {
        let mut response = deny(&store, &context, "proxy_login_throttled").await?;
        *response.status_mut() = StatusCode::TOO_MANY_REQUESTS;
        return Ok(response);
    }
    let Some(session) = authenticate_bound_person(
        &store,
        &context,
        principal_id,
        ExternalIdentityMechanism::TrustedProxy,
    )
    .await?
    else {
        return deny(&store, &context, "inactive_proxy_person").await;
    };
    external_session(session)
}
