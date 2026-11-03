//! Read authentication and mutations share the same live session/version checks.

use crate::credentials::_helpers::secrets::bearer_id;
use crate::credentials::main::sessions::authenticate_session_in::authenticate_session_in as authenticate;
use crate::credentials::models::{SessionAccess, SessionIdentity};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

pub async fn authenticate_session(
    store: &ControlStore,
    bearer: &str,
    access: SessionAccess<'_>,
) -> Result<Option<SessionIdentity>, ControlError> {
    if bearer_id(bearer).is_none() {
        return Ok(None);
    }
    let (transaction, identity) =
        authenticate(begin(store, TransactionMode::Read).await?, bearer, access).await?;
    commit(transaction).await?;
    Ok(identity)
}
