//! Live administrator checks, final-admin protection and audit share one lock.

use uuid::Uuid;

use crate::authorization::_helpers::administrator_changes::apply_change;
use crate::authorization::main::begin_authorized::begin_authorized;
use crate::credentials::models::{
    Action, AdministratorChange, AuditContext, DefaultRole, PermissionScope,
};
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

pub async fn change_administrator(
    store: &ControlStore,
    (context, actor): (&AuditContext, Uuid),
    (principal_id, change): (Uuid, AdministratorChange),
) -> Result<bool, ControlError> {
    let (transaction, context) = begin_authorized(
        store,
        (context, actor),
        &PermissionScope::action(
            match change {
                AdministratorChange::Grant | AdministratorChange::Revoke => Action::RoleAssign,
                AdministratorChange::Disable | AdministratorChange::Enable => Action::UserManage,
            },
            None,
        ),
        DefaultRole::None,
    )
    .await?;
    apply_change(transaction, &context, (principal_id, change)).await
}
