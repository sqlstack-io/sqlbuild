//! A build or plan that names jobs takes their build-wide settings where its own command line is silent.

use std::collections::BTreeMap;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyString;
use serde_json::Value;
use sqlbuild_cli::argv::main::accepted_settings_arguments::accepted_settings_arguments;

use crate::bindings::_helpers::cli::argv::parse_cli_invocation;
use crate::bindings::_helpers::compiled_project::job_settings::named_job_build_settings;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The commands whose selections take the settings of the jobs they name.
const JOB_SETTING_COMMANDS: [&str; 2] = ["build", "plan"];

/// The namespace `sqb <command> <arguments>` parses to.
fn parsed<'py>(
    py: Python<'py>,
    command: &str,
    arguments: &[String],
) -> PyResult<Bound<'py, PyAny>> {
    let argv: Vec<Bound<'py, PyString>> = std::iter::once(command)
        .chain(arguments.iter().map(String::as_str))
        .map(|argument| PyString::new(py, argument))
        .collect();
    let args: Bound<'py, PyAny> = native_field(&parse_cli_invocation(py, argv, false)?, "args")?;
    if args.is_none() {
        return Err(PyValueError::new_err(format!(
            "job settings do not parse as sqb {command} options: {}",
            arguments.join(" ")
        )));
    }
    Ok(args)
}

/// Set on `args` each build-wide setting of the jobs `selectors` name that its command line left
/// at the default, exactly as the same flags typed on the command line would.
pub(crate) fn apply_named_job_settings(
    args: &Bound<'_, PyAny>,
    selectors: &[String],
) -> PyResult<()> {
    let py: Python<'_> = args.py();
    let Some(command) = args.getattr("command")?.extract::<Option<String>>()? else {
        return Ok(());
    };
    if !JOB_SETTING_COMMANDS.contains(&command.as_str())
        || selectors.is_empty()
        || !args.getattr("job_settings")?.is_truthy()?
    {
        return Ok(());
    }
    let raw_project_dir: Bound<'_, PyAny> = args.getattr("project_dir")?;
    let path = py.import("pathlib")?.getattr("Path")?;
    let project_dir: Bound<'_, PyAny> = if raw_project_dir.is_none() {
        path.call_method0("cwd")?
    } else {
        path.call1((raw_project_dir,))?.call_method0("resolve")?
    };
    let target: Option<String> = args.getattr("target")?.extract()?;
    let settings: BTreeMap<String, Value> =
        named_job_build_settings(py, selectors, (&project_dir, target))?;
    if settings.is_empty() {
        return Ok(());
    }
    let defaults: Bound<'_, PyAny> = parsed(py, &command, &[])?;
    let mut silent: BTreeMap<String, Value> = BTreeMap::new();
    for (name, value) in settings {
        if !args.hasattr(name.as_str())?
            || args
                .getattr(name.as_str())?
                .eq(defaults.getattr(name.as_str())?)?
        {
            silent.insert(name, value);
        }
    }
    let flags: Vec<String> =
        accepted_settings_arguments(&command, &silent).map_err(PyValueError::new_err)?;
    if flags.is_empty() {
        return Ok(());
    }
    let applied: Bound<'_, PyAny> = parsed(py, &command, &flags)?;
    for (name, value) in applied
        .getattr("__dict__")?
        .call_method0("items")?
        .try_iter()?
        .map(|item| item.and_then(|item| item.extract::<(String, Bound<'_, PyAny>)>()))
        .collect::<PyResult<Vec<_>>>()?
    {
        if !value.eq(defaults.getattr(name.as_str())?)? {
            args.setattr(name.as_str(), value)?;
        }
    }
    Ok(())
}
