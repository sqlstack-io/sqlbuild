//! A service account holds one built-in role everywhere: the new role is given before the old ones go.

use uuid::Uuid;

use crate::authorization::_helpers::administrator_roles::assign_role_in;
use crate::authorization::_helpers::service_accounts::{
    begin_change, require_service_account, service_account_summary,
};
use crate::credentials::models::{
    Action, AuditContext, BuiltinRole, PermissionScope, RoleAssignmentRequest, RoleKey,
    ServiceAccountAuthority, ServiceAccountSummary,
};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// Gives `principal_id` `role` everywhere in place of its other built-in roles everywhere.
///
/// A person needs `role.assign` everywhere. Each role change is audited as people's are.
pub async fn set_service_account_role(
    store: &ControlStore,
    (context, authority): (&AuditContext, ServiceAccountAuthority),
    (principal_id, role): (Uuid, BuiltinRole),
) -> Result<ServiceAccountSummary, ControlError> {
    let (transaction, authorized) = begin_change(
        store,
        (context, authority),
        &[PermissionScope::action(Action::RoleAssign, None)],
    )
    .await?;
    let (transaction, _) = require_service_account(transaction, principal_id).await?;
    let (transaction, rows) = query(
        transaction,
        "SELECT role_key FROM sqb_role_assignments WHERE principal_id=$1 AND location_id IS NULL AND target IS NULL",
        vec![Parameter::Id(principal_id)],
    )
    .await?;
    let others: Vec<BuiltinRole> = rows
        .iter()
        .filter_map(|row| serde_json::from_value::<BuiltinRole>(row["role_key"].clone()).ok())
        .filter(|held| *held != role)
        .collect();
    let change = |role: BuiltinRole, revoke: bool| RoleAssignmentRequest {
        principal_id,
        role: RoleKey::Builtin(role),
        location_id: None,
        target: None,
        revoke,
    };
    assign_role_in(transaction, &authorized, &change(role, false)).await?;
    for held in others {
        let transaction = begin(store, TransactionMode::Write).await?;
        assign_role_in(transaction, &authorized, &change(held, true)).await?;
    }
    let (transaction, account) =
        service_account_summary(begin(store, TransactionMode::Read).await?, principal_id).await?;
    commit(transaction).await?;
    Ok(account)
}
