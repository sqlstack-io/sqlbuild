//! `sqb sensor try`: list a published sensor's recent events, or call its function from the project's code with one.

use std::path::Path;

use serde_json::json;
use sqlbuild_orchestration::sensors::constants::TRY_SENSOR_REQUEST;
use sqlbuild_orchestration::sensors::models::{SensorEventChoice, SensorTrial, SensorTrialEvents};
use sqlbuild_orchestration::store::errors::ControlError;
use uuid::Uuid;

use crate::argv::models::{Namespace, Stream};
use crate::native_commands::_helpers::command::decision::text;
use crate::native_commands::_helpers::command::terminal::write_output;
use crate::native_commands::_helpers::location::sensor_function::run_sensor_function;
use crate::native_commands::_helpers::sensor_trial::arguments::trial_arguments;
use crate::native_commands::_helpers::sensor_trial::events::trial_events;
use crate::native_commands::_helpers::sensor_trial::report::{
    call_warning, event_listing, outcome_report,
};
use crate::native_commands::constants::{SENSOR_COMMAND, SENSOR_FAILED_EXIT, SENSOR_USAGE_EXIT};
use crate::native_commands::models::{EventSource, LocationServe, SensorTrialArguments};

/// The exit status of `sqb sensor`, or `None` for another command; `script` starts the project's interpreter.
pub(crate) fn run_sensor_invocation(namespace: &Namespace, script: Option<&Path>) -> Option<i32> {
    if text(namespace, "command").as_deref() != Some(SENSOR_COMMAND) {
        return None;
    }
    let trial = match trial_arguments(namespace) {
        Ok(trial) => trial,
        Err(message) => {
            drop(write_output(&[(
                Stream::Stderr,
                &format!("sqb sensor: error: {message}\n"),
            )]));
            return Some(SENSOR_USAGE_EXIT);
        }
    };
    drop(write_output(&[(
        Stream::Stderr,
        &format!("Reading sensor {}'s events…\n", trial.sensor),
    )]));
    let read = match trial_events(&trial) {
        Ok(read) => read,
        Err(message) => {
            drop(write_output(&[(
                Stream::Stderr,
                &format!("Reading sensor events failed: {message}\n"),
            )]));
            return Some(SENSOR_FAILED_EXIT);
        }
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            drop(write_output(&[(
                Stream::Stderr,
                &format!("Sensor try failed: {error}\n"),
            )]));
            return Some(SENSOR_FAILED_EXIT);
        }
    };
    Some(runtime.block_on(try_sensor(&trial, read, script)))
}

async fn try_sensor(
    trial: &SensorTrialArguments,
    (location_id, events): (Uuid, SensorTrialEvents),
    script: Option<&Path>,
) -> i32 {
    if matches!(trial.choice, SensorEventChoice::Recent { .. }) {
        drop(write_output(&[
            (Stream::Stdout, &event_listing(&events)),
            (Stream::Stderr, "Sensor events read successfully\n"),
        ]));
        return 0;
    }
    let event = events.events.first();
    if events.when.is_some() && event.is_none() {
        drop(write_output(&[(
            Stream::Stderr,
            &format!(
                "No events match {}'s condition, so there is nothing to call it with.\n",
                events.sensor
            ),
        )]));
        return SENSOR_FAILED_EXIT;
    }
    let Some(script) = script else {
        drop(write_output(&[(
            Stream::Stderr,
            "Sensor try failed: the Python CLI that runs project code is missing\n",
        )]));
        return SENSOR_FAILED_EXIT;
    };
    drop(write_output(&[(
        Stream::Stderr,
        &call_warning(&events, (event, &trial.target)),
    )]));
    let options = LocationServe {
        project_dir: trial.project_dir.clone(),
        server_url: match &trial.source {
            EventSource::Server { origin, .. } => origin.clone(),
            EventSource::Development { .. } => String::new(),
        },
        location_id,
        target: trial.target.clone(),
        image_id: None,
        metadata_reconcile_seconds: 0,
        safety_sync_seconds: 0,
        dev_events: false,
    };
    let request = json!({
        TRY_SENSOR_REQUEST: SensorTrial {
            sensor: trial.sensor.clone(),
            target: trial.target.clone(),
            event: event.cloned(),
        },
    });
    let outcome = run_sensor_function(
        (&options, script),
        (&request, events.timeout_seconds),
        std::future::pending::<Result<(), ControlError>>(),
    )
    .await;
    match outcome {
        Ok(Some(outcome)) => {
            let (report, code) = outcome_report(&outcome, events.timeout_seconds);
            drop(write_output(&[(Stream::Stdout, &report)]));
            code
        }
        Ok(None) => SENSOR_FAILED_EXIT,
        Err(error) => {
            drop(write_output(&[(
                Stream::Stderr,
                &format!("Sensor try failed: {error}\n"),
            )]));
            SENSOR_FAILED_EXIT
        }
    }
}
