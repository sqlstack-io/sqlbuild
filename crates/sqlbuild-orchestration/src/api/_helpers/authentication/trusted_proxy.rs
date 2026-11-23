//! Each request's proxy username resolves through bindings or provisioning, and only from a trusted socket peer.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use axum::extract::{ConnectInfo, Request, State};
use axum::middleware::Next;
use axum::response::Response;
use axum::{Extension, Router, middleware};

use crate::api::_helpers::authentication::origins::parse_origin_with;
use crate::api::_helpers::authentication::password_sessions;
use crate::api::_helpers::authentication::password_transport::{self, deny, guard, single_header};
use crate::api::_helpers::authentication::proxy_settings::{ProxySettings, canonical_username};
use crate::api::_helpers::authentication::proxy_warnings::warn_plain_http_origin;
use crate::api::_helpers::authentication::service_keys;
use crate::api::constants::HTTP_SCHEME;
use crate::api::errors::ApiError;
use crate::api::models::{
    BrowserLoginMethod, BrowserOrigin, ProxyLogoutUrl, TrustedProxyAuthentication,
};
use crate::authorization::main::people::resolve_proxy_person::resolve_proxy_person;
use crate::authorization::models::ProxyPerson;
use crate::credentials::models::{AuditContext, AuthenticatedPerson, ServiceKeyIdentity};
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

pub(crate) fn router(
    store: &ControlStore,
    settings: TrustedProxyAuthentication,
) -> Result<Router<ControlStore>, ControlError> {
    let origin = BrowserOrigin(parse_origin_with(&settings.origin, "trusted-proxy", true)?);
    let settings = Arc::new(ProxySettings::new(settings)?);
    if origin.0.starts_with(&format!("{HTTP_SCHEME}://")) {
        warn_plain_http_origin(&origin.0);
    }
    let mut routes = Router::new()
        .merge(service_keys::keyed(
            store,
            password_sessions::person_routes()
                .route_layer(middleware::from_fn_with_state(store.clone(), authenticate)),
        ))
        .merge(password_sessions::session_routes(store))
        .merge(super::cli_login::router(BrowserLoginMethod::Proxy))
        .layer(middleware::from_fn(guard))
        .layer(middleware::from_fn(note_untrusted_sources));
    if let Some(url) = settings.logout_url.clone() {
        routes = routes.layer(Extension(ProxyLogoutUrl(url)));
    }
    Ok(routes.layer(Extension(origin)).layer(Extension(settings)))
}

/// Logs untrusted peers that send the username header before any check can refuse them.
async fn note_untrusted_sources(
    Extension(settings): Extension<Arc<ProxySettings>>,
    request: Request,
    next: Next,
) -> Response {
    let peer: Option<IpAddr> = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(peer)| peer.ip().to_canonical());
    if let Some(peer) = peer
        && !settings.trusted_peers.contains(&peer)
        && request.headers().contains_key(&settings.username_header)
    {
        settings.note_untrusted_source(peer);
    }
    next.run(request).await
}

/// A CLI session credential authenticates as a session; otherwise the trusted proxy's username does.
async fn authenticate(
    State(store): State<ControlStore>,
    Extension(settings): Extension<Arc<ProxySettings>>,
    mut request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    if request.extensions().get::<ServiceKeyIdentity>().is_some() {
        return Ok(next.run(request).await);
    }
    if password_transport::session_header(request.headers()) {
        return password_transport::authenticate(State(store), request, next).await;
    }
    let Some(mut context) = request.extensions().get::<AuditContext>().cloned() else {
        return Err(
            ControlError::Invalid("trusted authentication context is unavailable".into()).into(),
        );
    };
    if !context
        .ip_address
        .is_some_and(|peer| settings.trusted_peers.contains(&peer))
    {
        return deny(&context);
    }
    let Some(username) = single_header(request.headers(), settings.username_header.as_str())
        .and_then(canonical_username)
    else {
        return deny(&context);
    };
    let binding = settings.identities.get(&username).copied();
    let principal_id =
        match resolve_proxy_person(&store, &context, (&username, binding, settings.provision))
            .await?
        {
            ProxyPerson::Person(principal_id) => principal_id,
            ProxyPerson::Refused => return deny(&context),
            ProxyPerson::NameTaken => {
                settings.note_taken_username(&username);
                return deny(&context);
            }
        };
    context.actor = format!("principal:{principal_id}");
    context.granting_role = "trusted-proxy".into();
    let profile = settings.profile(request.headers());
    request.extensions_mut().insert(context);
    request.extensions_mut().insert(profile);
    request
        .extensions_mut()
        .insert(AuthenticatedPerson(principal_id, None));
    Ok(next.run(request).await)
}
