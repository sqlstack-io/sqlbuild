//! Live person eligibility, password versions and revocation apply to every browser request.

use crate::credentials::_helpers::secrets::{bearer_id, secret_matches};
use crate::credentials::models::{SessionAccess, SessionIdentity, StoredSession};
use crate::store::errors::ControlError;
use crate::store::main::{decode_row::decode_row, query::query};
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn authenticate_session_in(
    transaction: ControlTransaction,
    bearer: &str,
    access: SessionAccess<'_>,
) -> Result<(ControlTransaction, Option<SessionIdentity>), ControlError> {
    let Some(id) = bearer_id(bearer) else {
        return Ok((transaction, None));
    };
    let now = transaction.now;
    let (transaction, rows) = query(transaction,
        "SELECT s.session_id, s.principal_id, s.expires_at, s.secret_hash, s.csrf_hash FROM (\
         SELECT s.session_id,s.principal_id,s.expires_at,s.secret_hash,s.csrf_hash,s.revoked_at \
         FROM sqb_password_sessions s JOIN sqb_password_credentials c \
         ON c.principal_id=s.principal_id AND c.credential_id=s.credential_id \
         UNION ALL SELECT session_id,principal_id,expires_at,secret_hash,csrf_hash,revoked_at FROM sqb_external_sessions\
         ) s JOIN sqb_principals p ON p.principal_id=s.principal_id \
         WHERE s.session_id=$1 AND s.revoked_at IS NULL AND s.expires_at>$2 \
         AND p.kind='person' AND p.disabled_at IS NULL",
        vec![Parameter::Id(id), Parameter::Time(now)]).await?;
    let Some(row) = rows.into_iter().next() else {
        return Ok((transaction, None));
    };
    let session: StoredSession = decode_row(row)?;
    let csrf_matches = match access {
        SessionAccess::Read => true,
        SessionAccess::Write { csrf } => secret_matches(csrf, &session.csrf_hash),
    };
    Ok((
        transaction,
        (secret_matches(bearer, &session.secret_hash) && csrf_matches).then_some(session.identity),
    ))
}
