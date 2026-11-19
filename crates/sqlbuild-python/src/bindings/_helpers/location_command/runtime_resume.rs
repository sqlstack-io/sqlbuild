//! A completed sync's only warehouse write: resume the dynamic models whose holds expired.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyDict;
use serde_json::Value;
use sqlbuild_orchestration::warehouse_metadata::constants::DYNAMIC_RESUME_RECORDER;
use sqlbuild_orchestration::warehouse_metadata::models::{
    DynamicResumeLocation, DynamicResumed, ResumedHold,
};

use crate::bindings::_helpers::command_runtime::connection::project_connection_config;
use crate::bindings::_helpers::location_command::command::location_arguments;
use crate::bindings::_helpers::query_command::project::{
    ProjectAdapter, ProjectInputs, project_adapter,
};
use sqlbuild_python_front::native_host::main::resume_expired_dynamic_holds::{
    ResumeLocation, resume_expired_dynamic_holds,
};

/// Resume expired holds in `locations` over a writable session of its own, closed before it returns;
/// a location whose pause table still has its quiet token is not read.
pub(super) fn runtime_resume(
    py: Python<'_>,
    options: &Bound<'_, PyDict>,
    locations: &Value,
) -> PyResult<DynamicResumed> {
    let locations: Vec<DynamicResumeLocation> = serde_json::from_value(locations.clone())
        .map_err(|_| PyValueError::new_err("runtime received an invalid dynamic resume"))?;
    let locations: Vec<ResumeLocation> = locations
        .into_iter()
        .map(|resume| {
            (
                (resume.location.database, resume.location.schema),
                resume.quiet_token,
            )
        })
        .collect();
    let arguments = location_arguments(py, Some(options.clone()))?;
    let ProjectAdapter {
        project_dir,
        discovered,
        adapter,
    } = project_adapter(
        py,
        Some(arguments.value("project_dir")?),
        ProjectInputs::Configuration,
    )?;
    let config: Bound<'_, PyAny> = project_connection_config(
        &discovered,
        (&project_dir, &arguments.value("selected_target")?),
        (&py.None().into_bound(py), true),
        false,
    )?;
    let connection: Bound<'_, PyAny> = adapter.call_method1("connect", (config,))?;
    let recorded_by = format!(
        "{DYNAMIC_RESUME_RECORDER}{}",
        arguments.value("location_id")?.str()?
    );
    let resumed = resume_expired_dynamic_holds(
        (&adapter, &connection),
        &project_dir,
        &locations,
        &recorded_by,
    );
    let closed = adapter.call_method1("close", (&connection,));
    let (holds, quiet_tokens) = resumed?;
    closed?;
    Ok(DynamicResumed {
        holds: holds
            .into_iter()
            .map(|(run_id, relation)| ResumedHold { run_id, relation })
            .collect(),
        quiet_tokens,
    })
}
