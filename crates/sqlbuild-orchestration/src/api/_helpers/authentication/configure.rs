//! Build the configured sign-in routes and group mapping before migrations or HTTP startup.

use std::net::SocketAddr;

use axum::{Extension, Router, middleware};

use crate::api::_helpers::authentication::{development, oidc, password_sessions, trusted_proxy};
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
    let login_mode = match &login {
        HumanLogin::Oidc(_) => LoginMode::Oidc,
        HumanLogin::TrustedProxy(_) => LoginMode::TrustedProxy,
        HumanLogin::Development(_) => LoginMode::Disabled,
        _ => LoginMode::Password,
    };
    let (development, routes) = match login {
        HumanLogin::None => (None, Router::new()),
        HumanLogin::Development(scope) => {
            if !address.ip().is_loopback() {
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
