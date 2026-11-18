//! A build or plan naming jobs takes their build-wide settings where its command line is silent.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use pyo3::exceptions::{PyOSError, PyRuntimeError, PyUnicodeError, PyValueError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use serde_json::Value;
use sqlbuild_cli::argv::main::accepted_settings_arguments::accepted_settings_arguments;
use sqlbuild_cli::argv::main::given_setting_dests::given_setting_dests;
use sqlbuild_cli::dispatch::main::selector_file_selectors::selector_file_selectors;
use sqlbuild_discovery::constants::ORCHESTRATION_DIRECTORY;

use crate::bindings::_helpers::compiled_project::job_settings::{
    could_name_job, named_job_build_settings,
};
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::types::ErrorId;

/// Ends the options; job flags go before it so they are never read as selectors.
const OPTIONS_TERMINATOR: &str = "--";

/// The commands whose selections take the settings of the jobs they name.
const JOB_SETTING_COMMANDS: [&str; 2] = ["build", "plan"];

/// One parsed argument, `None` when unset.
fn argument<'py, T: pyo3::FromPyObject<'py>>(
    namespace: &Bound<'py, PyDict>,
    name: &str,
) -> PyResult<Option<T>> {
    Ok(namespace
        .get_item(name)?
        .map(|value| value.extract::<Option<T>>())
        .transpose()?
        .flatten())
}

/// The `--select` selectors and those of each `--select-file`.
fn selectors(namespace: &Bound<'_, PyDict>) -> PyResult<Vec<String>> {
    let mut selectors: Vec<String> = argument(namespace, "select")?.unwrap_or_default();
    let files: Vec<PathBuf> = argument(namespace, "select_file")?.unwrap_or_default();
    for path in files {
        let text: String = std::fs::read_to_string(&path)
            .map_err(|error| PyOSError::new_err(error.to_string()))?;
        selectors.extend(selector_file_selectors(&text));
    }
    Ok(selectors)
}

/// The project directory a command runs in, resolved as dispatch resolves it.
fn project_dir(namespace: &Bound<'_, PyDict>) -> PyResult<PathBuf> {
    let os_error = |error: std::io::Error| PyOSError::new_err(error.to_string());
    let Some(raw): Option<PathBuf> = argument(namespace, "project_dir")? else {
        return std::env::current_dir().map_err(os_error);
    };
    std::fs::canonicalize(&raw).or_else(|_| std::path::absolute(&raw).map_err(os_error))
}

/// `argv` plus the flags it leaves out of its named jobs' build-wide settings; `None` if unchanged.
fn named_job_argv(
    py: Python<'_>,
    argv: &[String],
    namespace: &Bound<'_, PyDict>,
) -> PyResult<Option<Vec<String>>> {
    let Some(command): Option<String> = argument(namespace, "command")? else {
        return Ok(None);
    };
    let job_settings: bool = argument(namespace, "job_settings")?.unwrap_or(false);
    if !JOB_SETTING_COMMANDS.contains(&command.as_str()) || !job_settings {
        return Ok(None);
    }
    let selectors: Vec<String> = selectors(namespace)?;
    if !selectors.iter().any(|selector| could_name_job(selector)) {
        return Ok(None);
    }
    let directory: PathBuf = project_dir(namespace)?;
    if !Path::new(&directory).join(ORCHESTRATION_DIRECTORY).is_dir() {
        return Ok(None);
    }
    let target: Option<String> = argument(namespace, "target")?;
    let settings: BTreeMap<String, Value> =
        named_job_build_settings(py, &selectors, (&directory, target))?;
    if settings.is_empty() {
        return Ok(None);
    }
    let given: BTreeSet<String> = given_setting_dests(argv).ok_or_else(|| {
        PyRuntimeError::new_err("cannot tell which settings the command line gives")
    })?;
    let silent: BTreeMap<String, Value> = settings
        .into_iter()
        .filter(|(name, _)| !given.contains(name))
        .collect();
    let flags: Vec<String> =
        accepted_settings_arguments(&command, &silent).map_err(PyValueError::new_err)?;
    if flags.is_empty() {
        return Ok(None);
    }
    let terminator: usize = argv
        .iter()
        .position(|argument| argument == OPTIONS_TERMINATOR)
        .unwrap_or(argv.len());
    let mut job_argv: Vec<String> = argv.to_vec();
    job_argv.splice(terminator..terminator, flags);
    Ok(Some(job_argv))
}

/// Whether the command meets `error` again, and reports it, when it runs.
fn reported_when_run(py: Python<'_>, error: &PyErr) -> PyResult<bool> {
    Ok(error.is_instance(py, &error_class(py, ErrorId::CliUser)?)
        || error.is_instance_of::<PyValueError>(py)
        || error.is_instance_of::<PyOSError>(py))
}

/// The argv to parse again with its named jobs' build-wide flags, or `None` to keep the parse.
pub(crate) fn job_settings_argv(
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
