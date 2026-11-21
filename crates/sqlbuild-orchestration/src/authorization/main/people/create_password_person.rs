//! An administrator adds a person who signs in with a password, optionally with a built-in role.

use uuid::Uuid;

use crate::authorization::_helpers::administrator_roles::assign_role_in;
use crate::authorization::_helpers::person_grants::{
    GrantDecision, decide_grant, person_role_rows,
};
use crate::authorization::main::decisions::authorize_person::authorize_person;
use crate::authorization::main::decisions::begin_authorized::begin_authorized;
use crate::credentials::main::identities::insert_principal::insert_principal;
use crate::credentials::main::sessions::hash_new_password::hash_new_password;
use crate::credentials::main::sessions::store_password::store_password;
use crate::credentials::models::{
    Action, AuditContext, BuiltinRole, DefaultRole, PermissionScope, RoleAssignmentRequest, RoleKey,
};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, ControlTransaction, Parameter, TransactionMode};

/// Creates `name` with `password` and `role` everywhere in one change; needs `user.manage`, and `role.assign` for a role.
pub async fn create_password_person(
    store: &ControlStore,
    (context, actor): (&AuditContext, Uuid),
    (name, password, role): (&str, &str, Option<BuiltinRole>),
) -> Result<Uuid, ControlError> {
    let scopes: Vec<PermissionScope> = required_scopes(role);
    if !holds_all(store, actor, &scopes).await? {
        let (transaction, _) = authorized(store, (context, actor), &scopes).await?;
        commit(transaction).await?;
    }
    let hash = hash_new_password(password).await?;
    let (transaction, authorized) = authorized(store, (context, actor), &scopes).await?;
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

/// `user.manage` everywhere, and `role.assign` everywhere when a role is given.
fn required_scopes(role: Option<BuiltinRole>) -> Vec<PermissionScope> {
    let assign = role.map(|_| PermissionScope::action(Action::RoleAssign, None));
    std::iter::once(PermissionScope::action(Action::UserManage, None))
        .chain(assign)
        .collect()
}

/// Whether `actor` holds every one of `scopes`, read without recording anything, so a refused request never pays for hashing.
async fn holds_all(
    store: &ControlStore,
    actor: Uuid,
    scopes: &[PermissionScope],
) -> Result<bool, ControlError> {
    let (transaction, rows) =
        person_role_rows(begin(store, TransactionMode::Read).await?, actor, None).await?;
    commit(transaction).await?;
    Ok(rows.is_some_and(|rows| {
        scopes.iter().all(|scope| {
            matches!(
                decide_grant(&rows, scope, DefaultRole::None),
                GrantDecision::Granted(_)
            )
        })
    }))
}

/// A write transaction authorized for every one of `scopes`, each refusal audited.
async fn authorized(
    store: &ControlStore,
    (context, actor): (&AuditContext, Uuid),
    scopes: &[PermissionScope],
) -> Result<(ControlTransaction, AuditContext), ControlError> {
    let (mut transaction, mut authorized) =
        begin_authorized(store, (context, actor), &scopes[0], DefaultRole::None).await?;
    for scope in &scopes[1..] {
        (transaction, authorized) =
            authorize_person(transaction, (context, actor), scope, DefaultRole::None).await?;
    }
    Ok((transaction, authorized))
}
