//! Revoke one session or every browser session without releasing the caller's write lock.

use serde_json::json;
use uuid::Uuid;

use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::models::AuditContext;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{Change, ControlTransaction, Parameter};

/// A missing session ID revokes all mechanisms; the caller audits its password/reset operation.
pub(crate) async fn revoke_session(
    transaction: ControlTransaction,
    context: &AuditContext,
    (principal_id, session_id): (Uuid, Option<Uuid>),
) -> Result<(ControlTransaction, usize), ControlError> {
    let now = transaction.now;
    let mut parameters = vec![Parameter::Time(now), Parameter::Id(principal_id)];
    let predicate = match session_id {
        Some(id) => {
            parameters.push(Parameter::Id(id));
            "AND session_id=$3"
        }
        None => "",
    };
    let (mut transaction, rows) = query(
        transaction,
        &format!("UPDATE sqb_sessions SET revoked_at=$1 WHERE principal_id=$2 {predicate} AND revoked_at IS NULL RETURNING session_id"),
        parameters,
    )
    .await?;
    let revoked = rows.len();
    if revoked == 0 {
        return Ok((transaction, 0));
    }
    if let Some(id) = session_id {
        transaction = append_audit(
            transaction,
            context,
            (Uuid::new_v5(&id, b"revoked"), "session.revoked"),
            (
                json!({"principal_id":principal_id}),
                json!({"session_id":id,"revoked":false}),
                json!({"session_id":id,"revoked":true}),
            ),
        )
        .await?;
    }
    transaction.changes.insert(Change::Credentials);
    Ok((transaction, revoked))
}
