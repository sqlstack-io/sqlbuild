//! Administrators who assign roles see what each role usable on a target grants and how many hold it.

use uuid::Uuid;

use crate::authorization::_helpers::role_overview::{read_role_facts, role_overviews};
use crate::authorization::main::begin_authorized::begin_authorized;
use crate::credentials::models::{
    Action, AuditContext, DefaultRole, PermissionScope, RoleOverview,
};
use crate::store::errors::ControlError;
use crate::store::main::commit::commit;
use crate::store::models::ControlStore;

/// Built-in roles first, then custom roles by key; requires `role.assign` everywhere.
pub async fn list_role_definitions(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Uuid),
    (location_id, target): (Uuid, &str),
) -> Result<Vec<RoleOverview>, ControlError> {
    let (transaction, _) = begin_authorized(
        store,
        (context, credentials),
        &PermissionScope::action(Action::RoleAssign, None),
        DefaultRole::None,
    )
    .await?;
    let (transaction, facts) = read_role_facts(transaction, (location_id, target)).await?;
    commit(transaction).await?;
    role_overviews(facts)
}
