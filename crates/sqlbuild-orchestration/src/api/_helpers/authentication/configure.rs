//! Build the configured sign-in routes and group mapping before migrations or HTTP startup.

use std::net::SocketAddr;

use axum::routing::get;
use axum::{Extension, Router, middleware};

use crate::api::_helpers::authentication::proxy_warnings::warn_plain_http_keys;
use crate::api::_helpers::authentication::{
    development, oidc, password_sessions, service_keys, trusted_proxy,
};
use crate::api::models::{ApiAuthentication, DevelopmentScope, GroupRole, HumanLogin, LoginMode};
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

pub(crate) async fn configure(
    store: &ControlStore,
    ApiAuthentication {
        login,
        default_role,
    }: ApiAuthentication,
    address: SocketAddr,
) -> Result<
    (
        Option<DevelopmentScope>,
        Router<ControlStore>,
        Vec<GroupRole>,
    ),
    ControlError,
> {
    let group_roles = match &login {
        HumanLogin::Oidc(settings) => settings.group_roles.clone(),
        _ => Vec::new(),
    };
    let origin = match &login {
        HumanLogin::Password(origin) => Some(origin.as_str()),
        HumanLogin::TrustedProxy(settings) => Some(settings.origin.as_str()),
        HumanLogin::Oidc(settings) => Some(settings.origin.as_str()),
        HumanLogin::None | HumanLogin::Development(_) => None,
    };
    if !matches!(login, HumanLogin::Development(_)) {
        warn_plain_http_keys(address, origin);
    }
    let login_mode = match &login {
        HumanLogin::Oidc(_) => LoginMode::Oidc,
        HumanLogin::TrustedProxy(_) => LoginMode::TrustedProxy,
        HumanLogin::Development(_) => LoginMode::Disabled,
        HumanLogin::None => LoginMode::Keys,
        HumanLogin::Password(_) => LoginMode::Password,
    };
    let (development, routes) = match login {
        HumanLogin::None => (
            None,
            service_keys::key_only(store, password_sessions::person_routes())
                .route("/api/auth/login-mode", get(service_keys::keys_only_mode)),
        ),
        HumanLogin::Development(scope) => {
            if !address.ip().is_loopback() && !scope.listens_beyond_loopback {
                return Err(ControlError::Invalid(
                    "disabled authentication requires a loopback listener".into(),
                ));
            }
            let person = scope.principal_id;
            (
                Some(scope),
                password_sessions::person_routes().route_layer(middleware::from_fn_with_state(
                    person,
                    development::authenticate,
                )),
            )
        }
        HumanLogin::Password(origin) => (None, password_sessions::router(store, origin)?),
        HumanLogin::TrustedProxy(settings) => (None, trusted_proxy::router(store, settings)?),
        HumanLogin::Oidc(settings) => (None, oidc::router(store, settings).await?),
    };
    Ok((
        development,
        routes
            .layer(Extension(default_role))
            .layer(Extension(login_mode)),
        group_roles,
    ))
}
