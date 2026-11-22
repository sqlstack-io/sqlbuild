//! A presented key is found by its hash; it works while unstopped, unexpired and its account enabled.

use chrono::{DateTime, Utc};

use crate::credentials::_helpers::service_keys::key_hash;
use crate::credentials::models::{ServiceKeyCheck, ServiceKeyIdentity, ServiceKeyState};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, decode_row::decode_row, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// What `key` is at `now`: a working key, a known key refused and why, or no key the store knows.
pub(crate) async fn authenticate_service_key(
    store: &ControlStore,
    key: &str,
    now: DateTime<Utc>,
) -> Result<ServiceKeyCheck, ControlError> {
    let (transaction, rows) = query(
        begin(store, TransactionMode::Read).await?,
        "SELECT k.key_id, k.principal_id, k.expires_at, k.revoked_at, k.last_used_at, p.disabled_at \
         FROM sqb_service_account_keys k JOIN sqb_principals p ON p.principal_id=k.principal_id \
         JOIN sqb_service_accounts s ON s.principal_id=k.principal_id WHERE k.secret_hash=$1",
        vec![Parameter::Text(key_hash(key))],
    )
    .await?;
    commit(transaction).await?;
    let Some(row) = rows.into_iter().next() else {
        return Ok(ServiceKeyCheck::Unknown);
    };
    let state: ServiceKeyState = decode_row(row)?;
    let refused = |reason| ServiceKeyCheck::Refused {
        principal_id: state.principal_id,
        key_id: state.key_id,
        reason,
    };
    Ok(if state.disabled_at.is_some() {
        refused("the service account is disabled")
    } else if state.revoked_at.is_some() {
        refused("the key was stopped")
    } else if state.expires_at.is_some_and(|end| end <= now) {
        refused("the key's rotation overlap ended")
    } else {
        ServiceKeyCheck::Accepted(ServiceKeyIdentity {
            principal_id: state.principal_id,
            key_id: state.key_id,
            last_used_at: state.last_used_at,
        })
    })
}
