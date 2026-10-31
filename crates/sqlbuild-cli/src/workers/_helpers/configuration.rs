//! Reject ambiguous or nonexistent installation-owned launcher scopes before claiming any run.

use std::collections::BTreeSet;

use sqlbuild_orchestration::store::errors::ControlError;
use uuid::Uuid;

use crate::workers::models::LocalLaunchers;

pub(in crate::workers) fn read_configuration(
    path: Option<&str>,
    scheduler_enabled: bool,
) -> Result<LocalLaunchers, ControlError> {
    let configuration: LocalLaunchers = match path {
        Some(path) => serde_json::from_slice(&std::fs::read(path)?)
            .map_err(|error| ControlError::Invalid(format!("launcher configuration: {error}")))?,
        None => LocalLaunchers {
            local: Vec::new(),
            concurrency_limits: Default::default(),
        },
    };
    if !scheduler_enabled && !configuration.local.is_empty() {
        return Err(ControlError::Invalid(
            "launchers require the embedded scheduler".into(),
        ));
    }
    let mut scopes: BTreeSet<(Uuid, &str)> = BTreeSet::new();
    for location in &configuration.local {
        if location.target.trim().is_empty()
            || !location.project_dir.is_absolute()
            || !location.project_dir.is_dir()
            || !location.executable.is_absolute()
            || !location.executable.is_file()
            || !scopes.insert((location.location_id, location.target.as_str()))
        {
            return Err(ControlError::Invalid("local launchers require unique location/target scopes and existing absolute project/executable paths".into()));
        }
    }
    Ok(configuration)
}
