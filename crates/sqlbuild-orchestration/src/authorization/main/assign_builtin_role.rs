//! Only live installation administrators may assign or revoke additive built-in roles.

use crate::authorization::_helpers::administrator_roles::assign_role_in;
use crate::authorization::main::authorize_session::authorize_session;
use crate::credentials::models::{
    Action, AdministratorAuthentication, AuditContext, DefaultRole, PermissionScope,
    RoleAssignmentRequest,
};
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::models::{ControlStore, TransactionMode};

pub async fn assign_builtin_role(
    store: &ControlStore,
    (context, credentials): (&AuditContext, (&str, &str)),
    request: &RoleAssignmentRequest,
    authentication: &AdministratorAuthentication,
) -> Result<bool, ControlError> {
    let (transaction, context) = authorize_session(
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
    assign_role_in(transaction, &context, request, authentication).await
}
