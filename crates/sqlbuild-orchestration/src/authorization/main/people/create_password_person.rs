//! An administrator adds a person who signs in with a password, optionally with a built-in role.

use uuid::Uuid;

use crate::authorization::_helpers::administrator_roles::assign_role_in;
use crate::authorization::main::decisions::authorize_person::authorize_person;
use crate::authorization::main::decisions::begin_authorized::begin_authorized;
use crate::credentials::main::identities::insert_principal::insert_principal;
use crate::credentials::main::sessions::hash_new_password::hash_new_password;
use crate::credentials::main::sessions::store_password::store_password;
use crate::credentials::models::{
    Action, AuditContext, BuiltinRole, DefaultRole, PermissionScope, RoleAssignmentRequest, RoleKey,
};
use crate::store::errors::ControlError;
use crate::store::main::{commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter};

/// Creates `name` with `password` and `role` everywhere in one change; needs `user.manage`, and `role.assign` for a role.
pub async fn create_password_person(
    store: &ControlStore,
    (context, actor): (&AuditContext, Uuid),
    (name, password, role): (&str, &str, Option<BuiltinRole>),
) -> Result<Uuid, ControlError> {
    let hash = hash_new_password(password).await?;
    let manage = PermissionScope::action(Action::UserManage, None);
    let (mut transaction, mut authorized) =
        begin_authorized(store, (context, actor), &manage, DefaultRole::None).await?;
    if role.is_some() {
        let assign = PermissionScope::action(Action::RoleAssign, None);
        (transaction, authorized) =
            authorize_person(transaction, (context, actor), &assign, DefaultRole::None).await?;
    }
    let (transaction, taken) = query(
        transaction,
        "SELECT principal_id FROM sqb_principals WHERE name=$1",
        vec![Parameter::Text(name.into())],
    )
    .await?;
    if !taken.is_empty() {
        return Err(ControlError::Invalid(format!(
            "a person named {name} already exists"
        )));
    }
    let (transaction, principal_id) =
        insert_principal(transaction, &authorized, (Uuid::now_v7(), name)).await?;
    let transaction = store_password(
        transaction,
        &authorized,
        (principal_id, hash),
        serde_json::Value::Null,
    )
    .await?;
    match role {
        Some(role) => {
            let request = RoleAssignmentRequest {
                principal_id,
                role: RoleKey::Builtin(role),
                location_id: None,
                target: None,
                revoke: false,
            };
            assign_role_in(transaction, &authorized, &request).await?;
        }
        None => commit(transaction).await?,
    }
    Ok(principal_id)
}
