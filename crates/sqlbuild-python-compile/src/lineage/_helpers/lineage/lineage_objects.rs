//! The Python lineage models and enums native lineage builds.

use sqlbuild_host::entries::models::ExecutorHost;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyDict;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;

const LOGGING_MODULE: &str = "logging";
const LINEAGE_LOGGER: &str = "sqlbuild.lineage";

/// A structured debug event on the `sqlbuild.lineage` logger for one model.
pub(crate) fn log_lineage_event(
    py: Python<'_>,
    message: &str,
    model_name: &str,
    error: Option<&str>,
) -> PyResult<()> {
    let logger: Bound<'_, PyAny> = py
        .import(LOGGING_MODULE)?
        .getattr("getLogger")?
        .call1((LINEAGE_LOGGER,))?;
    let fields: Bound<'_, PyDict> = PyDict::new(py);
    fields.set_item("logger", logger)?;
    fields.set_item("message", message)?;
    fields.set_item("sqlbuild_model", model_name)?;
    fields.set_item("sqlbuild_error", error)?;
    let _ = host_entry(py, ExecutorHost::LogDebugEvent)?.call((), Some(&fields))?;
    Ok(())
}
