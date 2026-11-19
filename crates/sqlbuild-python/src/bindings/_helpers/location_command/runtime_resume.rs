//! A completed sync's only warehouse write: resume the dynamic models whose holds expired.

use pyo3::prelude::{Bound, PyAnyMethods, PyResult, Python};
use pyo3::types::PyDict;
use serde_json::Value;
use sqlbuild_orchestration::warehouse_metadata::models::DynamicResumed;

use crate::bindings::_helpers::location_command::command::location_arguments;
use crate::bindings::_helpers::native_host::warehouse_metadata::close_history_session;
use crate::bindings::_helpers::native_host::warehouse_metadata::open_history_session;
use sqlbuild_python_front::native_host::main::resume_expired_dynamic_holds::resume_expired_dynamic_holds;

/// Resume expired holds in `locations` over a writable session of its own, closed before it returns.
pub(super) fn runtime_resume(
    py: Python<'_>,
    options: &Bound<'_, PyDict>,
    locations: &Value,
) -> PyResult<DynamicResumed> {
    let arguments = location_arguments(py, Some(options.clone()))?;
    let session = open_history_session(py, &arguments, true)?;
    let resumed = resume_expired_dynamic_holds(
        (&session.adapter, &session.connection),
        (
            &arguments.value("project_dir")?,
            &arguments.value("location_id")?.str()?.to_string(),
        ),
        locations,
    );
    close_history_session(session, None)?;
    resumed
}
