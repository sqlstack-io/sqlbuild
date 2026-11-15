//! Create or find the SQLBuild twin project a dbt command runs from.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyString, PyTuple};
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_core::dbt_command::constants::{
    JSON_OPTION, PROFILES_DIR_OPTION, PROJECT_DIR_OPTION, TARGET_OPTION, TWIN_PROJECT_DIR,
};
use sqlbuild_core::dbt_command::main::dbt_value_arg::dbt_value_arg;
use sqlbuild_core::dbt_command::main::forwarded_dbt_args::forwarded_dbt_args;
use sqlbuild_core::dbt_command::main::render_dbt_auto_init::render_dbt_auto_init;
use sqlbuild_core::dbt_command::models::ForwardedArg;

use crate::bindings::_helpers::build_progress::progress_reporters::DbtInitProgressReporter;
use crate::bindings::_helpers::dbt_command::init_project::build_dbt_init_project;
use crate::bindings::_helpers::dbt_command::support::{color_enabled, flag, path, stream};
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;

use sqlbuild_core::constants::PROJECT_CONFIG_FILENAME;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

fn project_config_found(directory: &Bound<'_, PyAny>) -> PyResult<bool> {
    let name: Bound<'_, PyAny> =
        pyo3::types::PyString::new(directory.py(), PROJECT_CONFIG_FILENAME).into_any();
    directory
        .call_method1("__truediv__", (name,))?
        .call_method0("exists")?
        .is_truthy()
}

/// `Path(raw).expanduser()`, resolved against `root` when relative.
fn resolved<'py>(root: &Bound<'py, PyAny>, raw: &str) -> PyResult<Bound<'py, PyAny>> {
    resolved_path(root, path(root.py(), raw)?.call_method0("expanduser")?)
}

/// The path resolved, joined under `root` first when relative.
fn resolved_path<'py>(
    root: &Bound<'py, PyAny>,
    expanded: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    if expanded.call_method0("is_absolute")?.is_truthy()? {
        return expanded.call_method0("resolve");
    }
    root.call_method1("__truediv__", (expanded,))?
        .call_method0("resolve")
}

/// The arguments with path values re-rooted at the original project directory.
fn forwarded_args<'py>(args: &[String], root: &Bound<'py, PyAny>) -> PyResult<Vec<String>> {
    forwarded_dbt_args(args)
        .into_iter()
        .map(|arg| match arg {
            ForwardedArg::Literal(text) => Ok(text),
            ForwardedArg::Path(raw) => resolved(root, &raw)?.call_method0("as_posix")?.extract(),
        })
        .collect()
}

/// The project directory and arguments a dbt command runs with, creating the twin when needed.
pub(crate) fn ensure_twin_project<'py>(
    effective_dir: Bound<'py, PyAny>,
    args: Vec<String>,
    no_color: bool,
) -> PyResult<(Bound<'py, PyAny>, Vec<String>)> {
    let py: Python<'py> = effective_dir.py();
    if project_config_found(&effective_dir)? {
        return Ok((effective_dir, args));
    }
    let dbt_project_dir: Bound<'py, PyAny> = match dbt_value_arg(&args, PROJECT_DIR_OPTION) {
        Some(raw) => resolved(&effective_dir, &raw)?,
        None => resolved_path(&effective_dir, effective_dir.clone())?,
    };
    let twin: Bound<'py, PyAny> = native_field(&dbt_project_dir, "parent")?
        .call_method1("__truediv__", (TWIN_PROJECT_DIR,))?;
    let forwarded: Vec<String> = forwarded_args(&args, &effective_dir)?;
    if project_config_found(&twin)? {
        return Ok((twin, forwarded));
    }
    let use_color: bool =
        !no_color && !args.iter().any(|arg| arg == JSON_OPTION) && color_enabled(py)?;
    let progress_stream: Bound<'py, PyAny> = stream(py, "stderr")?;
    let progress: Bound<'py, PyAny> = DbtInitProgressReporter::new(
        &progress_stream.clone(),
        flag(py, use_color).extract::<bool>()?,
    )
    .and_then(|value| pyo3::Bound::new(py, value))
    .map(pyo3::Bound::into_any)?;
    let callbacks: Bound<'py, PyAny> = new_record_with(
        py,
        RecordId::DbtInitProgressCallbacks,
        &[
            ("start", native_field(&progress, "start")?),
            ("complete", native_field(&progress, "complete")?),
        ],
    )?;
    let none: Bound<'py, PyAny> = py.None().into_bound(py);
    let profiles_dir: Bound<'py, PyAny> = match dbt_value_arg(&args, PROFILES_DIR_OPTION) {
        Some(raw) => path(py, &raw)?,
        None => none.clone(),
    };
    let target_name: Bound<'py, PyAny> = match dbt_value_arg(&args, TARGET_OPTION) {
        Some(raw) => PyString::new(py, &raw).into_any(),
        None => none.clone(),
    };
    let request: Bound<'py, PyAny> = new_record_with(
        py,
        RecordId::DbtInitRequest,
        &[
            ("cwd", effective_dir),
            ("dbt_project_dir", dbt_project_dir),
            ("profiles_dir", profiles_dir),
            ("profile_name", none.clone()),
            ("target_name", target_name),
            ("sqb_output_dir", twin),
            ("dry_run", flag(py, false)),
            ("overwrite", flag(py, false)),
            ("skip_dbt_debug", flag(py, true)),
            ("progress_callbacks", callbacks),
        ],
    )?;
    let result: Bound<'py, PyAny> = build_dbt_init_project(&request)?;
    let project_file: String = native_field(&result, "project_file")?.str()?.to_string();
    progress_stream.call_method1(
        "write",
        (render_dbt_auto_init(&project_file, CliStyle { use_color }),),
    )?;
    Ok((native_field(&result, "output_dir")?, forwarded))
}

/// The arguments as a Python tuple.
pub(crate) fn args_tuple<'py>(py: Python<'py>, args: &[String]) -> PyResult<Bound<'py, PyAny>> {
    Ok(PyTuple::new(py, args)?.into_any())
}
