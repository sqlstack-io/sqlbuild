//! Live person eligibility, password versions and revocation apply to every browser request.

use crate::credentials::_helpers::active_session::active_session;
use crate::credentials::_helpers::secrets::{bearer_id, secret_matches};
use crate::credentials::models::{SessionAccess, SessionIdentity};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

pub async fn authenticate_session(
    store: &ControlStore,
    bearer: &str,
    access: SessionAccess<'_>,
) -> Result<Option<SessionIdentity>, ControlError> {
    let Some(id) = bearer_id(bearer) else {
        return Ok(None);
    };
    let (transaction, session) =
        active_session(begin(store, TransactionMode::Read).await?, id).await?;
    commit(transaction).await?;
    let Some(session) = session else {
        return Ok(None);
    };
    let csrf_matches = match access {
        SessionAccess::Read => true,
        SessionAccess::Write { csrf } => secret_matches(csrf, &session.csrf_hash),
    };
    Ok((secret_matches(bearer, &session.secret_hash) && csrf_matches).then_some(session.identity))
}
