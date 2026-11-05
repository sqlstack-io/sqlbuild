//! Random bearer secrets are returned once; only their hashes and bounded scopes are durable.

use chrono::{DateTime, Utc};

use crate::credentials::_helpers::tokens::issue;
use crate::credentials::models::{AuditContext, CredentialScope, IssuedToken};
use crate::store::errors::ControlError;
use crate::store::main::write::write;
use crate::store::models::ControlStore;

/// The caller authorizes token management before entering this storage operation.
pub async fn issue_token(
    store: &ControlStore,
    context: &AuditContext,
    scope: CredentialScope,
    expires_at: DateTime<Utc>,
) -> Result<IssuedToken, ControlError> {
    write(store, |transaction| {
        issue(transaction, context, scope, expires_at)
    })
    .await
}
