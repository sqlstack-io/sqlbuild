//! The argv a build or plan naming jobs parses again with their build-wide flags.

use pyo3::exceptions::PyUnicodeError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyDict;

use crate::job_settings::_helpers::argv::{named_job_argv, reported_when_run};

/// The argv to parse again with its named jobs' build-wide flags, or `None` to keep the parse.
pub fn job_settings_argv(
    argv: &Bound<'_, PyAny>,
    namespace: &Bound<'_, PyDict>,
) -> PyResult<Option<Vec<String>>> {
    let py: Python<'_> = argv.py();
    let arguments: Vec<String> = match argv.extract() {
        Ok(arguments) => arguments,
        Err(undecodable) if undecodable.is_instance_of::<PyUnicodeError>(py) => return Ok(None),
        Err(error) => return Err(error),
    };
    match named_job_argv(py, &arguments, namespace) {
        Err(error) if reported_when_run(py, &error)? => Ok(None),
        result => result,
    }
}
