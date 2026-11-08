//! Administrators who assign roles see every assignment and SSO group mapping.

use uuid::Uuid;

use crate::authorization::main::begin_authorized::begin_authorized;
use crate::credentials::models::{
    Action, AuditContext, DefaultRole, PermissionScope, RoleHoldings,
};
use crate::store::errors::ControlError;
use crate::store::main::{commit::commit, decode_row::decode_row, query::query};
use crate::store::models::ControlStore;

/// Assignments by person name, then group mappings by group; requires `role.assign` everywhere.
pub async fn list_role_holdings(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Uuid),
) -> Result<RoleHoldings, ControlError> {
    let (transaction, _) = begin_authorized(
        store,
        (context, credentials),
        &PermissionScope::action(Action::RoleAssign, None),
        DefaultRole::None,
    )
    .await?;
    let (transaction, assignments) = query(
        transaction,
        "SELECT a.assignment_id, a.principal_id, p.name, a.role_key, a.location_id, a.target \
         FROM sqb_role_assignments a JOIN sqb_principals p ON p.principal_id=a.principal_id \
         ORDER BY p.name, a.role_key, a.location_id, a.target",
        Vec::new(),
    )
    .await?;
    let (transaction, group_roles) = query(
        transaction,
        "SELECT group_name, role_key, location_id, target FROM sqb_group_roles \
         ORDER BY group_name, role_key, location_id, target",
        Vec::new(),
    )
    .await?;
    commit(transaction).await?;
    Ok(RoleHoldings {
        assignments: assignments
            .into_iter()
            .map(decode_row)
            .collect::<Result<_, _>>()?,
        group_roles: group_roles
            .into_iter()
            .map(decode_row)
            .collect::<Result<_, _>>()?,
    })
}
