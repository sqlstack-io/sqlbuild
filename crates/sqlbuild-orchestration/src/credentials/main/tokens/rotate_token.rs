//! Replace one live credential without widening its scope or extending its lifetime.

use serde_json::json;
use uuid::Uuid;

use crate::credentials::_helpers::tokens::{issue, revoke};
use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::models::{AuditContext, IssuedToken, TokenIdentity};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, decode_row::decode_row, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// The trusted caller authorizes token management; the old UUID is not an authentication proof.
pub async fn rotate_token(
    store: &ControlStore,
    context: &AuditContext,
    token_id: Uuid,
) -> Result<IssuedToken, ControlError> {
    let transaction = begin(store, TransactionMode::Write).await?;
    let now = transaction.now;
    let (transaction, rows) = query(
        transaction,
        "SELECT token_id, scope_json AS scope, expires_at FROM sqb_access_tokens \
         WHERE token_id=$1 AND revoked_at IS NULL AND expires_at>$2",
        vec![Parameter::Id(token_id), Parameter::Time(now)],
    )
    .await?;
    let before: TokenIdentity = decode_row(
        rows.into_iter()
            .next()
            .ok_or_else(|| ControlError::Invalid("token is not active".into()))?,
    )?;
    let (transaction, _) = revoke(transaction, context, token_id).await?;
    let (transaction, issued) =
        issue(transaction, context, before.scope, before.expires_at).await?;
    let transaction = append_audit(
        transaction,
        context,
        (
            Uuid::new_v5(&issued.identity.token_id, b"rotated"),
            "token.rotated",
        ),
        (json!(before.scope), json!(before), json!(issued.identity)),
    )
    .await?;
    commit(transaction).await?;
    Ok(issued)
}
