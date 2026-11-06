//! Revocation and its security audit are one transaction, without a per-process token cache.

use uuid::Uuid;

use crate::credentials::_helpers::tokens::revoke;
use crate::credentials::models::AuditContext;
use crate::store::errors::ControlError;
use crate::store::main::write::write;
use crate::store::models::ControlStore;

/// Repeating a revocation does not create a second audit entry or rewrite its actor.
pub async fn revoke_token(
    store: &ControlStore,
    context: &AuditContext,
    token_id: Uuid,
) -> Result<bool, ControlError> {
    write(store, |transaction| revoke(transaction, context, token_id)).await
}
