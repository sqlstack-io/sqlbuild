//! Live administrator/session checks, final-admin protection, audit and revision share one lock.

use uuid::Uuid;

use crate::authorization::_helpers::administrator_changes::apply_change;
use crate::authorization::main::authorize_session::authorize_session;
use crate::credentials::models::{Action, AdministratorChange, AuditContext, PermissionScope};
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::models::{ControlStore, TransactionMode};

pub async fn change_administrator(
    store: &ControlStore,
    context: &AuditContext,
    (bearer, csrf): (&str, &str),
    (principal_id, change): (Uuid, AdministratorChange),
) -> Result<bool, ControlError> {
    let (transaction, context) = authorize_session(
        begin(store, TransactionMode::Write).await?,
        (context, (bearer, csrf)),
        &PermissionScope {
            action: match change {
                AdministratorChange::Grant | AdministratorChange::Revoke => Action::RoleAssign,
                AdministratorChange::Disable | AdministratorChange::Enable => Action::UserManage,
            },
            location_id: None,
            target: None,
            nodes: Default::default(),
        },
    )
    .await?;
    apply_change(transaction, &context, (principal_id, change)).await
}
