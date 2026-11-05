//! Installation administrator membership; callers establish bootstrap or authenticated authority.

use serde_json::json;
use uuid::Uuid;

use crate::authorization::main::deny_authorization::deny;
use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::models::AuditContext;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{Change, ControlTransaction, Parameter};

pub(crate) async fn grant_installation_admin(
    transaction: ControlTransaction,
    context: &AuditContext,
    principal_id: Uuid,
    source: &str,
) -> Result<(ControlTransaction, bool), ControlError> {
    let (transaction, people) = query(
        transaction,
        "SELECT principal_id FROM sqb_principals WHERE principal_id=$1 AND disabled_at IS NULL",
        vec![Parameter::Id(principal_id)],
    )
    .await?;
    if people.is_empty() {
        return Err(ControlError::Invalid(
            "administrator must be an active person".into(),
        ));
    }
    let assignment_id = Uuid::now_v7();
    let (mut transaction, rows) = query(
        transaction,
        "INSERT INTO sqb_role_assignments (assignment_id,principal_id,role_key,active_key) \
         VALUES ($1,$2,'admin',$3) ON CONFLICT (active_key) DO NOTHING RETURNING assignment_id",
        vec![
            Parameter::Id(assignment_id),
            Parameter::Id(principal_id),
            Parameter::Text(format!("{principal_id}:admin:installation")),
        ],
    )
    .await?;
    let applied = !rows.is_empty();
    if applied {
        transaction = append_audit(
            transaction,
            context,
            (assignment_id, "role.assigned"),
            (
                json!({"principal_id": principal_id}),
                json!(null),
                json!({"role_key":"admin", "source":source, "assignment_id":assignment_id}),
            ),
        )
        .await?;
        transaction.changes.insert(Change::Credentials);
    }
    Ok((transaction, applied))
}

/// Another active installation administrator exists; a lost one is restored with grant-admin.
pub(crate) async fn remaining_administrator(
    transaction: ControlTransaction,
    principal_id: Uuid,
) -> Result<(ControlTransaction, bool), ControlError> {
    let (transaction, rows) = query(transaction,
        "SELECT COUNT(*) AS administrators FROM sqb_role_assignments a JOIN sqb_principals p USING(principal_id) \
         WHERE a.role_key='admin' AND a.location_id IS NULL AND a.target IS NULL \
         AND p.disabled_at IS NULL AND p.principal_id<>$1",
        vec![Parameter::Id(principal_id)]).await?;
    Ok((
        transaction,
        rows.first()
            .and_then(|row| row["administrators"].as_i64())
            .is_some_and(|count| count > 0),
    ))
}

pub(crate) async fn deny_change(
    transaction: ControlTransaction,
    context: &AuditContext,
    principal_id: Uuid,
    reason: &str,
) -> Result<bool, ControlError> {
    deny(
        transaction,
        context,
        json!({"principal_id":principal_id}),
        reason,
    )
    .await
}
