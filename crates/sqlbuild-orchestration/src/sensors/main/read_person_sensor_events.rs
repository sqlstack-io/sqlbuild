//! The events a sensor's condition matches, for `sqb sensor try` by a person who may list the target.

use uuid::Uuid;

use crate::authorization::main::decisions::authorize_listing::authorize_listing;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::sensors::_helpers::events::trial::sensor_events;
use crate::sensors::models::{SensorEventChoice, SensorTrialEvents};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

/// The chosen events of the published sensor, or why there are none to choose; for a person who may list the target.
pub async fn read_person_sensor_events(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Uuid),
    ((location_id, target), (name, choice)): ((Uuid, &str), (&str, &SensorEventChoice)),
    (default_role, origin): (DefaultRole, Option<&str>),
) -> Result<Result<SensorTrialEvents, String>, ControlError> {
    let (transaction, _) = authorize_listing(
        begin(store, TransactionMode::Write).await?,
        (context, credentials),
        (location_id, target),
        default_role,
    )
    .await?;
    let (transaction, events) =
        sensor_events(transaction, ((location_id, target), (name, choice)), origin).await?;
    commit(transaction).await?;
    Ok(events)
}
