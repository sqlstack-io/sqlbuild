//! The published sensor and its chosen events, from a server or the local `sqb dev` store.

use sqlbuild_orchestration::credentials::main::locations::development_location::existing_development_location;
use sqlbuild_orchestration::sensors::main::read_sensor_events::read_sensor_events as read_store_events;
use sqlbuild_orchestration::sensors::models::SensorTrialEvents;
use sqlbuild_orchestration::store::main::open_store::open_store;
use uuid::Uuid;

use crate::native_commands::models::{EventSource, SensorTrialArguments};
use crate::person_login::main::read_sensor_events::read_sensor_events;

/// The location the events belong to, and the sensor with the events the trial chose.
pub(crate) fn trial_events(
    trial: &SensorTrialArguments,
) -> Result<(Uuid, SensorTrialEvents), String> {
    match &trial.source {
        EventSource::Server {
            origin,
            ca,
            location_id,
        } => read_sensor_events(
            (origin, ca.as_deref()),
            (*location_id, &trial.target, &trial.sensor),
            &trial.choice,
        )
        .map(|events| (*location_id, events))
        .map_err(|error| error.to_string()),
        EventSource::Development { store_url } => tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?
            .block_on(development_events(trial, store_url)),
    }
}

async fn development_events(
    trial: &SensorTrialArguments,
    store_url: &str,
) -> Result<(Uuid, SensorTrialEvents), String> {
    let store = open_store(store_url)
        .await
        .map_err(|error| format!("can't open the sqb dev store at {store_url}: {error}"))?;
    let location_id = existing_development_location(
        &store,
        &trial.project_dir.to_string_lossy(),
        &trial.target,
    )
    .await
    .map_err(|error| error.to_string())?
    .ok_or_else(|| {
        format!(
            "the sqb dev store at {store_url} has no location for this project and target {}; start sqb dev, or pass --server",
            trial.target
        )
    })?;
    let events = read_store_events(
        &store,
        ((location_id, &trial.target), (&trial.sensor, &trial.choice)),
        None,
    )
    .await
    .map_err(|error| error.to_string())??;
    Ok((location_id, events))
}
