//! Only live installation administrators may assign or revoke additive built-in roles.

use uuid::Uuid;

use crate::authorization::_helpers::administrator_roles::assign_role_in;
use crate::authorization::main::authorize_person::authorize_person;
use crate::credentials::models::{
    Action, AuditContext, DefaultRole, PermissionScope, RoleAssignmentRequest,
};
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::models::{ControlStore, TransactionMode};

pub async fn assign_builtin_role(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Uuid),
    request: &RoleAssignmentRequest,
) -> Result<bool, ControlError> {
    let (transaction, context) = authorize_person(
        begin(store, TransactionMode::Write).await?,
        (context, credentials),
        &PermissionScope {
            action: Action::RoleAssign,
            location_id: None,
            target: None,
            nodes: Default::default(),
        },
        DefaultRole::None,
    )
    .await?;
    assign_role_in(transaction, &context, request).await
}
