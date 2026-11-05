//! All person-session consumers share live person, expiry, revocation and password-version checks.

use uuid::Uuid;

use crate::credentials::models::StoredSession;
use crate::store::errors::ControlError;
use crate::store::main::{decode_row::decode_row, query::query};
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn active_session(
    transaction: ControlTransaction,
    id: Uuid,
) -> Result<(ControlTransaction, Option<StoredSession>), ControlError> {
    let now = transaction.now;
    let (transaction, rows) = query(transaction,
        "SELECT s.session_id,s.principal_id,s.expires_at,s.secret_hash,s.csrf_hash,s.credential_id \
         FROM sqb_sessions s JOIN sqb_principals p ON p.principal_id=s.principal_id \
         LEFT JOIN sqb_password_credentials c ON c.principal_id=s.principal_id AND c.credential_id=s.credential_id \
         WHERE s.session_id=$1 AND s.revoked_at IS NULL AND s.expires_at>$2 \
         AND p.disabled_at IS NULL AND (s.credential_id IS NULL OR c.credential_id IS NOT NULL)",
        vec![Parameter::Id(id), Parameter::Time(now)]).await?;
    Ok((
        transaction,
        rows.into_iter().next().map(decode_row).transpose()?,
    ))
}
