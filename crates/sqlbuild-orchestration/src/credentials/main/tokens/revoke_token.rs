//! Revocation and its security audit are one transaction, without a per-process token cache.

use uuid::Uuid;

use crate::credentials::_helpers::tokens::revoke;
use crate::credentials::models::AuditContext;
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::models::{ControlStore, TransactionMode};

/// Repeating a revocation does not create a second audit entry or rewrite its actor.
pub async fn revoke_token(
    store: &ControlStore,
    context: &AuditContext,
    token_id: Uuid,
) -> Result<bool, ControlError> {
    let transaction = begin(store, TransactionMode::Write).await?;
    let (transaction, revoked) = revoke(transaction, context, token_id).await?;
    commit(transaction).await?;
    Ok(revoked)
}
