//! People read a target's health only with view on every current node, like its listings.

use uuid::Uuid;

use crate::authorization::main::decisions::authorize_listing::authorize_listing;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::declarations::main::current_jobs::current_jobs;
use crate::declarations::models::ServerPoolCaps;
use crate::platform::_helpers::health::pool_cap_warnings::{
    pool_cap_observation, pool_cap_warnings,
};
use crate::platform::_helpers::health::recent_health::recent_health;
use crate::platform::models::LocationHealthObservation;
use crate::snapshots::main::reads::read_current_record::read_current_record;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

/// The location's newest observations for this target only, newest first, with this server's pool-cap warning on the current snapshot.
pub async fn read_person_location_health(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Uuid),
    (location_id, target): (Uuid, &str),
    (default_role, caps): (DefaultRole, &ServerPoolCaps),
) -> Result<Vec<LocationHealthObservation>, ControlError> {
    let (transaction, _) = authorize_listing(
        begin(store, TransactionMode::Write).await?,
        (context, credentials),
        (location_id, target),
        default_role,
    )
    .await?;
    let (transaction, mut observations) =
        recent_health(transaction, location_id, Some(target)).await?;
    let (transaction, snapshot) = read_current_record(transaction, (location_id, target)).await?;
    let (transaction, jobs) = current_jobs(transaction, (location_id, target), caps).await?;
    commit(transaction).await?;
    if let Some(warning) =
        snapshot.and_then(|snapshot| pool_cap_observation(&snapshot, pool_cap_warnings(&jobs)))
    {
        observations.push(warning);
        observations.sort_by_key(|observation| std::cmp::Reverse(observation.received_at));
    }
    Ok(observations)
}
