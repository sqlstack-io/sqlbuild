//! Live password versions invalidate all earlier sessions without per-process caches.

use crate::credentials::_helpers::secrets::{bearer_id, secret_matches};
use crate::credentials::models::{SessionAccess, SessionIdentity, StoredSession};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, decode_row::decode_row, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

pub async fn authenticate_session(
    store: &ControlStore,
    bearer: &str,
    access: SessionAccess<'_>,
) -> Result<Option<SessionIdentity>, ControlError> {
    let Some(id) = bearer_id(bearer) else {
        return Ok(None);
    };
    let transaction = begin(store, TransactionMode::Read).await?;
    let now = transaction.now;
    let (transaction, rows) = query(transaction,
        "SELECT s.session_id, s.principal_id, s.expires_at, s.secret_hash, s.csrf_hash \
         FROM sqb_password_sessions s JOIN sqb_principals p ON p.principal_id=s.principal_id \
         JOIN sqb_password_credentials c ON c.principal_id=s.principal_id AND c.credential_id=s.credential_id \
         WHERE s.session_id=$1 AND s.revoked_at IS NULL AND s.expires_at>$2 \
         AND p.kind='person' AND p.disabled_at IS NULL",
        vec![Parameter::Id(id), Parameter::Time(now)]).await?;
    commit(transaction).await?;
    let Some(row) = rows.into_iter().next() else {
        return Ok(None);
    };
    let session: StoredSession = decode_row(row)?;
    let csrf_matches = match access {
        SessionAccess::Read => true,
        SessionAccess::Write { csrf } => secret_matches(csrf, &session.csrf_hash),
    };
    Ok((secret_matches(bearer, &session.secret_hash) && csrf_matches).then_some(session.identity))
}
