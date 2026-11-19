//! Resume the dynamic models whose holds expired, in each state location a sync read.

use crate::discovery::main::project::configured_heartbeat_grace::configured_heartbeat_grace;
use pyo3::IntoPyObjectExt;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use serde_json::Value;
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_orchestration::warehouse_metadata::constants::DYNAMIC_RESUME_RECORDER;
use sqlbuild_orchestration::warehouse_metadata::models::{
    DynamicResumeLocation, DynamicResumed, ResumedHold,
};
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;

/// The holds silent past `[dynamic] heartbeat_grace` it closed, and each location's quiet token.
pub fn resume_expired_dynamic_holds<'py>(
    (adapter, connection): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (project_dir, location_id): (&Bound<'py, PyAny>, &str),
    locations: &Value,
) -> PyResult<DynamicResumed> {
    let py = adapter.py();
    let locations: Vec<DynamicResumeLocation> = serde_json::from_value(locations.clone())
        .map_err(|_| PyValueError::new_err("runtime received an invalid dynamic resume"))?;
    let grace_seconds = configured_heartbeat_grace(project_dir)? as f64;
    let resume: Bound<'py, PyAny> = host_entry(py, ExecutorHost::ResumeExpiredDynamicHolds)?;
    let mut resumed = DynamicResumed {
        holds: Vec::new(),
        quiet_tokens: Vec::new(),
    };
    for request in locations {
        let location = (request.location.database, request.location.schema);
        let outcome: Bound<'py, PyAny> = call_with(
            &resume,
            &[
                ("adapter", adapter.clone()),
                ("connection", connection.clone()),
                ("location", location.into_bound_py_any(py)?),
                ("grace_seconds", grace_seconds.into_bound_py_any(py)?),
                (
                    "recorded_by",
                    format!("{DYNAMIC_RESUME_RECORDER}{location_id}").into_bound_py_any(py)?,
                ),
                ("quiet_token", request.quiet_token.into_bound_py_any(py)?),
            ],
        )?;
        for hold in outcome.getattr("resumed")?.try_iter()? {
            let hold: Bound<'py, PyAny> = hold?;
            let relation: Bound<'py, PyAny> = hold.getattr("relation")?;
            let mut parts: Vec<String> = Vec::new();
            for part in ["database", "schema", "name"] {
                parts.extend(relation.getattr(part)?.extract::<Option<String>>()?);
            }
            resumed.holds.push(ResumedHold {
                run_id: hold.getattr("run_id")?.extract()?,
                relation: parts.join("."),
            });
        }
        resumed
            .quiet_tokens
            .push(outcome.getattr("quiet_token")?.extract()?);
    }
    Ok(resumed)
}
