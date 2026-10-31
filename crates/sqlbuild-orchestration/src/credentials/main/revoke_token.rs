//! Revocation and its security audit are one transaction, without a per-process token cache.

use serde_json::json;
use uuid::Uuid;

use crate::credentials::main::append_audit::append_audit;
use crate::credentials::models::AuditContext;
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// Repeating a revocation does not create a second audit entry or rewrite its actor.
pub async fn revoke_token(
    store: &ControlStore,
    context: &AuditContext,
    token_id: Uuid,
) -> Result<bool, ControlError> {
    let transaction = begin(store, TransactionMode::Write).await?;
    let now = transaction.now;
    let (transaction, rows) = query(transaction,
        "UPDATE sqb_access_tokens SET revoked_at=$1 WHERE token_id=$2 AND revoked_at IS NULL RETURNING scope_json",
        vec![Parameter::Time(now), Parameter::Id(token_id)]).await?;
    let Some(row) = rows.into_iter().next() else {
        commit(transaction).await?;
        return Ok(false);
    };
    let scope = row
        .get("scope_json")
        .cloned()
        .ok_or_else(|| ControlError::Invalid("stored token scope is missing".into()))?;
    commit(
        append_audit(
            transaction,
            context,
            (Uuid::new_v5(&token_id, b"revoked"), "token.revoked"),
            (
                scope,
                json!({"token_id": token_id, "revoked": false}),
                json!({"token_id": token_id, "revoked": true}),
            ),
        )
        .await?,
    )
    .await?;
    Ok(true)
}
