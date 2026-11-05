//! Only live installation administrators may assign or revoke additive built-in roles.

use uuid::Uuid;

use crate::authorization::_helpers::administrator_roles::assign_role_in;
use crate::authorization::main::begin_authorized::begin_authorized;
use crate::credentials::models::{
    Action, AuditContext, DefaultRole, PermissionScope, RoleAssignmentRequest,
};
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

pub async fn assign_builtin_role(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Uuid),
    request: &RoleAssignmentRequest,
) -> Result<bool, ControlError> {
    let (transaction, context) = begin_authorized(
        store,
        (context, credentials),
        &PermissionScope::action(Action::RoleAssign, None),
        DefaultRole::None,
    )
    .await?;
    assign_role_in(transaction, &context, request).await
}
