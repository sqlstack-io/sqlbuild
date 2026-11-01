//! Logout preserves session history and commits its security audit with revocation.

use serde_json::json;
use uuid::Uuid;

use crate::credentials::main::append_audit::append_audit;
use crate::credentials::models::{AuditContext, SessionIdentity};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{Change, ControlStore, Parameter, TransactionMode};

pub async fn revoke_session(
    store: &ControlStore,
    context: &AuditContext,
    identity: &SessionIdentity,
) -> Result<bool, ControlError> {
    let transaction = begin(store, TransactionMode::Write).await?;
    let now = transaction.now;
    let (transaction, rows) = query(
        transaction,
        "UPDATE sqb_password_sessions SET revoked_at=$1 WHERE session_id=$2 \
         AND principal_id=$3 AND revoked_at IS NULL RETURNING session_id",
        vec![
            Parameter::Time(now),
            Parameter::Id(identity.session_id),
            Parameter::Id(identity.principal_id),
        ],
    )
    .await?;
    if rows.is_empty() {
        commit(transaction).await?;
        return Ok(false);
    }
    let mut transaction = append_audit(
        transaction,
        context,
        (
            Uuid::new_v5(&identity.session_id, b"revoked"),
            "session.revoked",
        ),
        (
            json!({"principal_id": identity.principal_id}),
            json!({"session_id": identity.session_id, "revoked": false}),
            json!({"session_id": identity.session_id, "revoked": true}),
        ),
    )
    .await?;
    transaction.changes.insert(Change::Credentials);
    commit(transaction).await?;
    Ok(true)
}
