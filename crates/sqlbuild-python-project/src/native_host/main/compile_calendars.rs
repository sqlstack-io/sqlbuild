//! Each calendar request's trigger, with each (calendar, timezone) pair evaluated once.

use crate::native_host::_helpers::orchestration_triggers::calendar_times;
use crate::native_host::models::CalendarRequest;
use chrono::{Timelike, Utc};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyBool;
use sqlbuild_host::entries::models::UserCodeHost;
use sqlbuild_orchestration::declarations::models::{Definitions, Schedule, TriggerDefinition};
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::main::within::within;
use sqlbuild_python_base::native_host::main::host_entry::host_entry as entry;
use sqlbuild_python_front::discovery::main::python_files::discover_provider_classes::discover_provider_classes;
use sqlbuild_python_front::discovery::main::python_files::provider_by_name::provider_by_name;
use sqlbuild_python_front::discovery::models::SecretPolicy;
use std::collections::BTreeMap;
use std::path::Path;

/// Each calendar request's trigger, with each (calendar, timezone) pair evaluated once.
pub fn compile_calendars(
    py: Python<'_>,
    project_dir: &Path,
    (calendars, requests): (&BTreeMap<String, Bound<'_, PyAny>>, Vec<CalendarRequest>),
    mut definitions: Definitions,
) -> PyResult<Definitions> {
    if requests.is_empty() {
        return Ok(definitions);
    }
    let providers = discover_provider_classes(
        imported(py, "pathlib", "Path")?.call1((project_dir,))?,
        SecretPolicy::Defer.name(),
    )?;
    let by_name = provider_by_name(&providers)?;
    let session = call_with(
        &entry(py, UserCodeHost::BuildProviderSession)?,
        &[
            ("discovered_providers", providers),
            ("allow_shared", PyBool::new(py, false).to_owned().into_any()),
        ],
    )?;
    let now = Utc::now();
    let start = now
        .with_nanosecond(now.timestamp_subsec_micros() * 1_000)
        .ok_or_else(|| PyValueError::new_err("calendar clock is out of range"))?;
    within(&session, |_| {
        let mut evaluated: BTreeMap<(String, String), Schedule> = BTreeMap::new();
        for request in requests {
            let key = (request.calendar.clone(), request.timezone.clone());
            let schedule = match evaluated.get(&key) {
                Some(schedule) => schedule.clone(),
                None => {
                    let calendar = calendars.get(&request.calendar).ok_or_else(|| {
                        PyValueError::new_err(format!(
                            "calendar '{}' is not declared by a calendar module",
                            request.calendar
                        ))
                    })?;
                    let schedule =
                        calendar_times((calendar, &request.timezone), &session, &by_name, start)
                            .map_err(|error| {
                                PyValueError::new_err(format!(
                                    "calendar '{}': {error}",
                                    request.calendar
                                ))
                            })?;
                    evaluated.insert(key, schedule.clone());
                    schedule
                }
            };
            let mut trigger = TriggerDefinition {
                schedule,
                ..request.trigger
            };
            trigger.starts_at.get_or_insert(start);
            definitions
                .triggers
                .insert(trigger.job_key.clone(), trigger);
        }
        Ok(definitions)
    })
}
