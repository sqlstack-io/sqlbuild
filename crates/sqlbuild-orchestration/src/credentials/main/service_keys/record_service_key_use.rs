//! A key's last use moves forward at most once a minute, so busy callers don't write on every request.

use chrono::{DateTime, Duration, Utc};

use crate::credentials::constants::KEY_USE_INTERVAL_SECONDS;
use crate::credentials::models::ServiceKeyIdentity;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// Records that `identity`'s key served a request at `now`, unless that was already recorded within the interval.
pub(crate) async fn record_service_key_use(
    store: &ControlStore,
    identity: ServiceKeyIdentity,
    now: DateTime<Utc>,
) -> Result<(), ControlError> {
    let interval = Duration::seconds(KEY_USE_INTERVAL_SECONDS);
    if identity
        .last_used_at
        .is_some_and(|last| now - last < interval)
    {
        return Ok(());
    }
    let (transaction, _) = query(
        begin(store, TransactionMode::Write).await?,
        "UPDATE sqb_service_account_keys SET last_used_at=$1 WHERE key_id=$2 \
         AND (last_used_at IS NULL OR last_used_at<$3)",
        vec![
            Parameter::Time(now),
            Parameter::Id(identity.key_id),
            Parameter::Time(now - interval),
        ],
    )
    .await?;
    commit(transaction).await
}
