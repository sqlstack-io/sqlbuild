//! Apply eligibility/membership changes without releasing the caller's authorization lock.

use serde_json::json;
use uuid::Uuid;

use crate::authorization::_helpers::administrators::{
    deny_change, grant_installation_admin, remaining_administrator,
};
use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::models::{AdministratorChange, AuditContext};
use crate::store::errors::ControlError;
use crate::store::main::{commit::commit, query::query};
use crate::store::models::{Change, ControlTransaction, Parameter};

pub(crate) async fn apply_change(
    transaction: ControlTransaction,
    context: &AuditContext,
    (principal_id, change): (Uuid, AdministratorChange),
) -> Result<bool, ControlError> {
    let (mut transaction, rows) = query(transaction,
        "SELECT p.disabled_at,a.assignment_id FROM sqb_principals p LEFT JOIN sqb_role_assignments a \
         ON p.principal_id=a.principal_id AND a.role_key='admin' AND a.location_id IS NULL \
         AND a.target IS NULL WHERE p.principal_id=$1",
        vec![Parameter::Id(principal_id)]).await?;
    let Some(person) = rows.first() else {
        return deny_change(
            transaction,
            context,
            principal_id,
            "target must be a person",
        )
        .await;
    };
    let active = person["disabled_at"].is_null();
    let administrator = !person["assignment_id"].is_null();
    if active
        && administrator
        && matches!(
            change,
            AdministratorChange::Revoke | AdministratorChange::Disable
        )
    {
        let other;
        (transaction, other) = remaining_administrator(transaction, principal_id).await?;
        if !other {
            return deny_change(
                transaction,
                context,
                principal_id,
                "cannot remove or disable the final installation administrator",
            )
            .await;
        }
    }
    let now = transaction.now;

    let changed;
    let action = match change {
        AdministratorChange::Grant => {
            if !active {
                return deny_change(
                    transaction,
                    context,
                    principal_id,
                    "administrator must be an active person",
                )
                .await;
            }
            let applied;
            (transaction, applied) =
                grant_installation_admin(transaction, context, principal_id, "manual").await?;
            commit(transaction).await?;
            return Ok(applied);
        }
        AdministratorChange::Revoke => {
            (transaction, changed) = query(
                transaction,
                "DELETE FROM sqb_role_assignments WHERE principal_id=$1 AND role_key='admin' \
                 AND location_id IS NULL AND target IS NULL RETURNING assignment_id",
                vec![Parameter::Id(principal_id)],
            )
            .await?;
            "role.revoked"
        }
        AdministratorChange::Disable | AdministratorChange::Enable => {
            let disable = change == AdministratorChange::Disable;
            let predicate = if disable { "IS NULL" } else { "IS NOT NULL" };
            (transaction, changed) = query(transaction,
                &format!("UPDATE sqb_principals SET disabled_at=$1 WHERE principal_id=$2 AND disabled_at {predicate} RETURNING principal_id"),
                vec![Parameter::OptionalTime(disable.then_some(now)),Parameter::Id(principal_id)]).await?;
            if disable {
                "principal.disabled"
            } else {
                "principal.enabled"
            }
        }
    };
    let applied = !changed.is_empty();
    if applied {
        transaction = append_audit(transaction, context, (Uuid::now_v7(), action),
            (json!({"principal_id":principal_id}), json!({"administrator":administrator,"enabled":active}),
             json!({"administrator":administrator && change!=AdministratorChange::Revoke,"enabled":change==AdministratorChange::Enable || (active && change!=AdministratorChange::Disable)}))).await?;
        transaction.changes.insert(Change::Credentials);
    }
    commit(transaction).await?;
    Ok(applied)
}
