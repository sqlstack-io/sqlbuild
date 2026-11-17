//! A trusted proxy's username resolves to a bound or provisioned person, provisioning newcomers as viewers.

use uuid::Uuid;

use crate::authorization::_helpers::proxy_people::{
    principal_enabled, provision_viewer, proxy_principal_id,
};
use crate::authorization::models::ProxyPerson;
use crate::credentials::models::AuditContext;
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

/// The enabled person a canonical `username` names: its binding, else its provisioned person.
pub(crate) async fn resolve_proxy_person(
    store: &ControlStore,
    context: &AuditContext,
    (username, binding, provision): (&str, Option<Uuid>, bool),
) -> Result<ProxyPerson, ControlError> {
    let principal_id = binding.unwrap_or_else(|| proxy_principal_id(username));
    let refused_or = |enabled: bool| {
        if enabled {
            ProxyPerson::Person(principal_id)
        } else {
            ProxyPerson::Refused
        }
    };
    match principal_enabled(store, principal_id).await? {
        Some(enabled) => return Ok(refused_or(enabled)),
        None if binding.is_some() || !provision => return Ok(ProxyPerson::Refused),
        None => {}
    }
    let mut context = context.clone();
    context.actor = format!("principal:{principal_id}");
    context.granting_role = "trusted-proxy-provisioning".into();
    match provision_viewer(store, &context, (principal_id, username)).await {
        Ok(true) => Ok(ProxyPerson::Person(principal_id)),
        Ok(false) => Ok(ProxyPerson::NameTaken),
        Err(error) => match principal_enabled(store, principal_id).await? {
            Some(enabled) => Ok(refused_or(enabled)),
            None => Err(error),
        },
    }
}
