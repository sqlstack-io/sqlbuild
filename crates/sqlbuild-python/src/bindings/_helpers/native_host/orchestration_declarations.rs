//! The public authoring records enter native compilation here, never scheduler execution.

use std::collections::HashSet;
use std::path::Path;

use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods, PyTuple};
use sqlbuild_host::entries::models::UserCodeHost;

use crate::bindings::_helpers::native_host::host::entry;

/// JSON authoring values cross once; the interpreter rejects circular user containers safely.
pub(crate) fn authored_setting(value: &Bound<'_, PyAny>) -> PyResult<serde_json::Value> {
    let text: String = value
        .py()
        .import("json")?
        .call_method1("dumps", (value,))?
        .extract()?;
    serde_json::from_str(&text).map_err(|error| PyValueError::new_err(error.to_string()))
}

/// Only module execution crosses into Python; discovery and collection stay native.
pub(crate) fn module_jobs<'py>(
    py: Python<'py>,
    file_path: &Path,
    project_dir: &Path,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let options = PyDict::new(py);
    options.set_item("file_path", file_path)?;
    options.set_item("project_dir", project_dir)?;
    let module = entry(py, UserCodeHost::LoadOrchestrationModule)?.call((), Some(&options))?;
    let job_class = py.import("sqlbuild.orchestration")?.getattr("Job")?;
    let attributes = module.getattr("__dict__")?.cast_into::<PyDict>()?;
    let mut pending: Vec<Bound<'py, PyAny>> = attributes.values().iter().collect();
    let mut seen: HashSet<usize> = HashSet::new();
    let mut jobs: Vec<Bound<'py, PyAny>> = Vec::new();
    while let Some(value) = pending.pop() {
        if !seen.insert(value.as_ptr() as usize) {
            continue;
        }
        if value.is_instance(&job_class)? {
            jobs.push(value);
        } else if value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>() {
            pending.extend(value.try_iter()?.collect::<PyResult<Vec<_>>>()?);
        }
    }
    Ok(jobs)
}

pub(crate) fn validate_jobs(
    py: Python<'_>,
    declarations: &[Bound<'_, PyAny>],
) -> Result<(), PyErr> {
    let job = py.import("sqlbuild.orchestration")?.getattr("Job")?;
    for declaration in declarations {
        if !declaration.is_instance(&job)? {
            return Err(PyTypeError::new_err(
                "snapshot declarations must be sqlbuild.orchestration.Job objects",
            ));
        }
    }
    Ok(())
}
