//! Random bearer secrets are returned once; only their hashes and bounded scopes are durable.

use chrono::{DateTime, Utc};

use crate::credentials::_helpers::tokens::issue;
use crate::credentials::models::{AuditContext, CredentialScope, IssuedToken};
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::models::{ControlStore, TransactionMode};

/// The caller authorizes token management before entering this storage operation.
pub async fn issue_token(
    store: &ControlStore,
    context: &AuditContext,
    scope: CredentialScope,
    expires_at: DateTime<Utc>,
) -> Result<IssuedToken, ControlError> {
    let transaction = begin(store, TransactionMode::Write).await?;
    let (transaction, issued) = issue(transaction, context, scope, expires_at).await?;
    commit(transaction).await?;
    Ok(issued)
}
