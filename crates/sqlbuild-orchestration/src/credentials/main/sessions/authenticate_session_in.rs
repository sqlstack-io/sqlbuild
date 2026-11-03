//! Live person eligibility, password versions and revocation apply to every browser request.

use crate::credentials::_helpers::active_session::active_session;
use crate::credentials::_helpers::secrets::{bearer_id, secret_matches};
use crate::credentials::models::{SessionAccess, SessionIdentity};
use crate::store::errors::ControlError;
use crate::store::models::ControlTransaction;

pub(crate) async fn authenticate_session_in(
    transaction: ControlTransaction,
    bearer: &str,
    access: SessionAccess<'_>,
) -> Result<(ControlTransaction, Option<SessionIdentity>), ControlError> {
    let Some(id) = bearer_id(bearer) else {
        return Ok((transaction, None));
    };
    let (transaction, session) = active_session(transaction, id).await?;
    let Some(session) = session else {
        return Ok((transaction, None));
    };
    let csrf_matches = match access {
        SessionAccess::Read => true,
        SessionAccess::Write { csrf } => secret_matches(csrf, &session.csrf_hash),
    };
    Ok((
        transaction,
        (secret_matches(bearer, &session.secret_hash) && csrf_matches).then_some(session.identity),
    ))
}
