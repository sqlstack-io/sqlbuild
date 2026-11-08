//! People read a target's health only with view on every current node, like its listings.

use uuid::Uuid;

use crate::authorization::main::authorize_listing::authorize_listing;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::platform::_helpers::recent_health::recent_health;
use crate::platform::models::LocationHealthObservation;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

/// The location's newest observations for this target only, newest first.
pub async fn read_person_location_health(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Uuid),
    (location_id, target): (Uuid, &str),
    default_role: DefaultRole,
) -> Result<Vec<LocationHealthObservation>, ControlError> {
    let (transaction, _) = authorize_listing(
        begin(store, TransactionMode::Write).await?,
        (context, credentials),
        (location_id, target),
        default_role,
    )
    .await?;
    let (transaction, observations) = recent_health(transaction, location_id).await?;
    commit(transaction).await?;
    Ok(observations
        .into_iter()
        .filter(|observation| observation.report.target == target)
        .collect())
}
