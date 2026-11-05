//! Build the configured sign-in routes before migrations or HTTP startup.

use std::net::SocketAddr;

use axum::{Extension, Router};

use crate::api::_helpers::authentication::{oidc, password_sessions, trusted_proxy};
use crate::api::models::{ApiAuthentication, DevelopmentScope, HumanLogin};
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

pub(crate) async fn configure(
    store: &ControlStore,
    ApiAuthentication {
        login,
        default_role,
    }: ApiAuthentication,
    address: SocketAddr,
) -> Result<(Option<DevelopmentScope>, Router<ControlStore>), ControlError> {
    let (development, routes) = match login {
        HumanLogin::None => (None, Router::new()),
        HumanLogin::Development(scope) => {
            if !address.ip().is_loopback() {
                return Err(ControlError::Invalid(
                    "disabled authentication requires a loopback listener".into(),
                ));
            }
            (Some(scope), Router::new())
        }
        HumanLogin::Password(origin) => (None, password_sessions::router(store, origin)?),
        HumanLogin::TrustedProxy(settings) => (None, trusted_proxy::router(store, settings)?),
        HumanLogin::Oidc(settings) => (None, oidc::router(store, settings).await?),
    };
    Ok((development, routes.layer(Extension(default_role))))
}
