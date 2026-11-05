//! Live administrator checks, final-admin protection and audit share one lock.

use uuid::Uuid;

use crate::authorization::_helpers::administrator_changes::apply_change;
use crate::authorization::main::authorize_person::authorize_person;
use crate::credentials::models::{
    Action, AdministratorChange, AuditContext, DefaultRole, PermissionScope,
};
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::models::{ControlStore, TransactionMode};

pub async fn change_administrator(
    store: &ControlStore,
    (context, actor): (&AuditContext, Uuid),
    (principal_id, change): (Uuid, AdministratorChange),
) -> Result<bool, ControlError> {
    let (transaction, context) = authorize_person(
        begin(store, TransactionMode::Write).await?,
        (context, actor),
        &PermissionScope {
            action: match change {
                AdministratorChange::Grant | AdministratorChange::Revoke => Action::RoleAssign,
                AdministratorChange::Disable | AdministratorChange::Enable => Action::UserManage,
            },
            location_id: None,
            target: None,
            nodes: Default::default(),
        },
        DefaultRole::None,
    )
    .await?;
    apply_change(transaction, &context, (principal_id, change)).await
}
