//! The events a sensor's condition matches, from a store the caller owns outright, such as the local `sqb dev` store.

use uuid::Uuid;

use crate::sensors::_helpers::events::trial::sensor_events;
use crate::sensors::models::{SensorEventChoice, SensorTrialEvents};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

/// The chosen events from a store the caller owns outright, such as the local `sqb dev` store.
pub async fn read_sensor_events(
    store: &ControlStore,
    ((location_id, target), (name, choice)): ((Uuid, &str), (&str, &SensorEventChoice)),
    origin: Option<&str>,
) -> Result<Result<SensorTrialEvents, String>, ControlError> {
    let transaction = begin(store, TransactionMode::Read).await?;
    let (transaction, events) =
        sensor_events(transaction, ((location_id, target), (name, choice)), origin).await?;
    commit(transaction).await?;
    Ok(events)
}
