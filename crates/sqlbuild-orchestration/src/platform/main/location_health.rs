//! Read bounded recent observations and instance liveness; consumers derive freshness.

use uuid::Uuid;

use crate::platform::_helpers::recent_health::recent_health;
use crate::platform::models::LocationHealthObservation;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

/// Every target's observations, or only `target`'s.
pub async fn location_health(
    store: &ControlStore,
    (location_id, target): (Uuid, Option<&str>),
) -> Result<Vec<LocationHealthObservation>, ControlError> {
    let (transaction, observations) = recent_health(
        begin(store, TransactionMode::Read).await?,
        location_id,
        target,
    )
    .await?;
    commit(transaction).await?;
    Ok(observations)
}
