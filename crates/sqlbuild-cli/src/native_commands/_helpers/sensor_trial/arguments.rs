//! A trial's arguments from the parsed command line, or the usage error that stops it.

use std::path::PathBuf;

use sqlbuild_orchestration::sensors::models::SensorEventChoice;

use crate::argv::models::{Namespace, Value};
use crate::native_commands::_helpers::command::decision::{flag, text};
use crate::native_commands::main::effective_target::effective_target;
use crate::native_commands::models::{EventSource, SensorTrialArguments};

/// The trial `namespace` asks for; the error is the message of a usage error.
pub(crate) fn trial_arguments(namespace: &Namespace) -> Result<SensorTrialArguments, String> {
    let sensor = text(namespace, "sensor_name").unwrap_or_default();
    if sensor.trim().is_empty() {
        return Err("sensor try needs the sensor's name".into());
    }
    let project_dir: PathBuf =
        std::fs::canonicalize(text(namespace, "project_dir").unwrap_or_else(|| ".".into()))
            .map_err(|error| format!("project directory is unreadable: {error}"))?;
    let target = effective_target(&project_dir, text(namespace, "target"))?
        .ok_or("the project names no default target; pass --target")?;
    let limit: u32 = match namespace.get("sensor_limit") {
        Some(Value::Int(limit)) => match limit.parse::<u32>() {
            Ok(limit) if limit > 0 => limit,
            _ => return Err("--limit must be a positive whole number".into()),
        },
        _ => 10,
    };
    let choice = if flag(namespace, "sensor_latest") {
        SensorEventChoice::Latest
    } else if let Some(event_id) = text(namespace, "sensor_event") {
        SensorEventChoice::Event { event_id }
    } else if let Some(run) = text(namespace, "sensor_run") {
        SensorEventChoice::Run {
            run_id: run.parse().map_err(|_| "--run must be a run UUID")?,
        }
    } else {
        SensorEventChoice::Recent { limit }
    };
    let source = match text(namespace, "login_server") {
        Some(origin) => EventSource::Server {
            origin,
            ca: text(namespace, "login_ca").map(PathBuf::from),
            location_id: text(namespace, "person_location")
                .ok_or("--location-id is required with --server")?
                .parse()
                .map_err(|_| "--location-id must be a UUID")?,
        },
        None => EventSource::Development {
            store_url: text(namespace, "development_store_url").unwrap_or_else(|| {
                project_dir
                    .join(".sqlbuild/control.sqlite3")
                    .to_string_lossy()
                    .into_owned()
            }),
        },
    };
    Ok(SensorTrialArguments {
        sensor,
        project_dir,
        target,
        choice,
        source,
    })
}
