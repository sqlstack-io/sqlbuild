//! Validate authentication modes before migrations or HTTP startup.

use std::net::SocketAddr;

use axum::Router;

use crate::api::_helpers::authentication::password_sessions::router;
use crate::api::models::{ApiAuthentication, DevelopmentScope};
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

pub(crate) fn configure(
    store: &ControlStore,
    authentication: ApiAuthentication,
    address: SocketAddr,
) -> Result<(Option<DevelopmentScope>, Router<ControlStore>), ControlError> {
    let ApiAuthentication {
        development_scope,
        password_origin,
    } = authentication;
    if development_scope.is_some() && password_origin.is_some() {
        return Err(ControlError::Invalid(
            "disabled and password authentication cannot be combined".into(),
        ));
    }
    if development_scope.is_some() && !address.ip().is_loopback() {
        return Err(ControlError::Invalid(
            "disabled authentication requires a loopback listener".into(),
        ));
    }
    Ok((development_scope, router(store, password_origin)?))
}
