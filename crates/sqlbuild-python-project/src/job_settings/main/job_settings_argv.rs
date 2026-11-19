//! The argv a build or plan naming jobs parses again with their build-wide flags.

use std::collections::BTreeSet;

use pyo3::exceptions::PyUnicodeError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyString};
use sqlbuild_cli::argv::main::given_setting_dests::given_setting_dests;

use crate::job_settings::_helpers::argv::{named_job_argv, reported_when_run};
use crate::job_settings::types::JobSettingsArgv;

/// The settings the command line gives, and the argv to parse again with its jobs' flags, if any.
pub fn job_settings_argv<'py>(
    argv: &Bound<'py, PyAny>,
    namespace: &Bound<'py, PyDict>,
) -> PyResult<JobSettingsArgv<'py>> {
    let py: Python<'py> = argv.py();
    let arguments: Vec<String> = match argv.extract() {
        Ok(arguments) => arguments,
        Err(undecodable) if undecodable.is_instance_of::<PyUnicodeError>(py) => {
            return Ok((BTreeSet::new(), None));
        }
        Err(error) => return Err(error),
    };
    let (given, job_argv) = match named_job_argv(py, &arguments, namespace) {
        Err(error) if reported_when_run(py, &error)? => {
            (given_setting_dests(&arguments).unwrap_or_default(), None)
        }
        result => result?,
    };
    let Some(job_argv) = job_argv else {
        return Ok((given, None));
    };
    let strings: Vec<Bound<'py, PyString>> = job_argv
        .iter()
        .map(|argument| PyString::new(py, argument))
        .collect();
    Ok((given, Some(strings)))
}
