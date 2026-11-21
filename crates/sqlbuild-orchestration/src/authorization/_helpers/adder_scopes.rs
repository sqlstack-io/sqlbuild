//! The permissions adding a password person needs, read cheaply before hashing and then decided under lock.

use uuid::Uuid;

use crate::authorization::_helpers::person_grants::{
    GrantDecision, decide_grant, person_role_rows,
};
use crate::authorization::main::decisions::authorize_person::authorize_person;
use crate::authorization::main::decisions::begin_authorized::begin_authorized;
use crate::credentials::models::{Action, AuditContext, BuiltinRole, DefaultRole, PermissionScope};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, ControlTransaction, TransactionMode};

/// `user.manage` everywhere, and `role.assign` everywhere when a role is given.
pub(crate) fn required_scopes(role: Option<BuiltinRole>) -> Vec<PermissionScope> {
    let assign = role.map(|_| PermissionScope::action(Action::RoleAssign, None));
    std::iter::once(PermissionScope::action(Action::UserManage, None))
        .chain(assign)
        .collect()
}

/// Whether `actor` holds every one of `scopes`, read without recording anything, so a refused request never pays for hashing.
pub(crate) async fn holds_all(
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
pub(crate) async fn authorized(
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
