//! A service account holds one built-in role everywhere: the new role and the old ones' removal commit together.

use uuid::Uuid;

use crate::authorization::_helpers::administrator_roles::write_scoped_role;
use crate::authorization::_helpers::administrators::grant_installation_admin;
use crate::authorization::_helpers::service_accounts::{
    begin_change, finish_change, require_service_account, revoke_other_roles,
};
use crate::credentials::models::{
    Action, AuditContext, BuiltinRole, PermissionScope, RoleAssignmentRequest, RoleKey,
    ServiceAccountAuthority, ServiceAccountSummary,
};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter};

/// Gives `principal_id` `role` everywhere in place of its other built-in roles; a person needs `role.assign`.
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
    let transaction = require_service_account(transaction, principal_id).await?;
    let (transaction, enabled) = query(
        transaction,
        "SELECT principal_id FROM sqb_principals WHERE principal_id=$1 AND disabled_at IS NULL",
        vec![Parameter::Id(principal_id)],
    )
    .await?;
    if enabled.is_empty() {
        return Err(ControlError::Invalid(
            "enable the service account before changing its role".into(),
        ));
    }
    let transaction = revoke_other_roles(transaction, &authorized, (principal_id, role)).await?;
    let (transaction, _) = if role == BuiltinRole::Admin {
        grant_installation_admin(transaction, &authorized, principal_id, "manual").await?
    } else {
        let request = RoleAssignmentRequest {
            principal_id,
            role: RoleKey::Builtin(role),
            location_id: None,
            target: None,
            revoke: false,
        };
        write_scoped_role(transaction, &authorized, &request).await?
    };
    finish_change(transaction, principal_id).await
}
