//! Check the values of TOML jobs and groups at compile, with the checks publication applies.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use pyo3::prelude::Python;
use serde_json::Value;
use sqlbuild_analysis::graph::main::parse_selector::parse_selector;
use sqlbuild_cli::argv::main::job_settings_arguments::job_settings_arguments;
use sqlbuild_discovery::models::{DiscoveryFailure, FailureKind};
use sqlbuild_discovery::orchestration_files::models::{
    FilePosition, OrchestrationInventory, Positioned, RunFile, ScheduleSource,
};
use sqlbuild_orchestration::scheduler::main::parse_cron::parse_cron;
use sqlbuild_orchestration::store::errors::ControlError;

use crate::bindings::_helpers::native_host::orchestration_declarations::run_resources;

/// A `D017` at `position` of `path`, worded as discovery words its own.
type ValueError = (FilePosition, String);

/// The first invalid schedule, setting, resource limit or selector, as `D017` at its file, line and column.
pub(crate) fn check_orchestration_values(
    py: Python<'_>,
    inventory: &OrchestrationInventory,
) -> Result<(), DiscoveryFailure> {
    let runs = inventory
        .jobs
        .iter()
        .map(|(path, job)| (path, &job.run))
        .chain(inventory.groups.iter().map(|(path, group)| (path, &group.run)));
    for (path, run) in runs {
        schedule_values(run).map_err(|error| failure(path, error))?;
    }
    for (path, job) in &inventory.jobs {
        for selector in std::iter::once(&job.select).chain(&job.exclude) {
            selector_syntax(selector).map_err(|error| failure(path, error))?;
        }
        let mut settings: BTreeMap<String, Value> = BTreeMap::new();
        for (key, value) in &job.settings {
            settings.insert(key.clone(), value.clone());
            job_settings_arguments(&BTreeMap::from([(key.clone(), value.clone())]))
                .map_err(|message| failure(path, (job.setting_positions[key], message)))?;
        }
        if let Some(first) = job.setting_positions.values().min() {
            job_settings_arguments(&settings)
                .map_err(|message| failure(path, (*first, message)))?;
        }
        if let Some(resources) = &job.resources {
            run_resources(&job.run.name.value, &resources.value).map_err(|error| {
                failure(path, (resources.position, error.value(py).to_string()))
            })?;
        }
    }
    Ok(())
}

fn failure(path: &str, (FilePosition { line, column }, message): ValueError) -> DiscoveryFailure {
    DiscoveryFailure {
        kind: FailureKind::Orchestration,
        message: format!("{path}:{line}:{column}: {message}"),
        help: None,
    }
}

fn schedule_values(run: &RunFile) -> Result<(), ValueError> {
    let Some(schedule) = &run.schedule else {
        return Ok(());
    };
    let timezone: &Positioned<String> = &schedule.timezone;
    timezone.value.parse::<Tz>().map_err(|error| {
        (
            timezone.position,
            format!("schedule timezone is not an IANA time zone: {error}"),
        )
    })?;
    if let ScheduleSource::Cron(cron) = &schedule.source {
        parse_cron(&cron.value, &timezone.value).map_err(|error| {
            let reason: String = match error {
                ControlError::Invalid(reason) => reason,
                other => other.to_string(),
            };
            (cron.position, format!("schedule cron: {reason}"))
        })?;
    }
    if let Some(starts_at) = &schedule.starts_at {
        serde_json::from_value::<DateTime<Utc>>(Value::String(starts_at.value.clone())).map_err(
            |error| {
                (
                    starts_at.position,
                    format!("schedule starts_at must be an RFC 3339 timestamp: {error}"),
                )
            },
        )?;
    }
    Ok(())
}

fn selector_syntax(selector: &Positioned<String>) -> Result<(), ValueError> {
    let parts = selector
        .value
        .split_whitespace()
        .flat_map(|token| token.split(','));
    for part in parts {
        parse_selector(part).map_err(|error| (selector.position, error.message))?;
    }
    Ok(())
}
