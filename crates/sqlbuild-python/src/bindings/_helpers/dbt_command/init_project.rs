//! Build or validate a SQLBuild project from a dbt project, its profile and `dbt debug`.

use crate::bindings::_helpers::dbt_interop::profile_connection::profile_error;
use std::io;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyTuple};
use pyo3::{IntoPyObjectExt, pyfunction, wrap_pyfunction};
use sqlbuild_core::dbt_process::main::run_dbt_process::run_dbt_process;
use sqlbuild_core::dbt_process::models::DbtProcessOutput;
use sqlbuild_core::text::main::python_strip::python_strip;

use sqlbuild_host::entries::models::ThirdPartyHost;

use crate::bindings::_helpers::dbt_interop::profile::build_dbt_init_project_toml;
use crate::bindings::_helpers::dbt_interop::profile::normalize_dbt_profile_connection;
use crate::bindings::_helpers::dbt_interop::profile_load::{
    load_dbt_project_metadata, load_selected_dbt_profile_output,
};
use crate::bindings::_helpers::dbt_interop::runner::launch_os_error;
use sqlbuild_config::dbt_profile::models::DbtProjectMetadata;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::types::CompilerDetach;

use sqlbuild_core::constants::PROJECT_CONFIG_FILENAME as PROJECT_FILE;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
const DEFAULT_OUTPUT_DIR: &str = "sqlbuild_project";
const PROFILES_FILE: &str = "profiles.yml";
const PROFILES_DIR_ENV_VAR: &str = "DBT_PROFILES_DIR";
const DEBUG_SKIPPED: &str =
    "dbt debug was skipped because the dbt executable was not found on PATH.";
const DEBUG_FAILED: &str = "dbt debug failed; generated SQLBuild config anyway.";

/// The resolved dbt project folder and profiles folder of one init request.
struct InitPaths<'py> {
    dbt_project_dir: Bound<'py, PyAny>,
    profiles_dir: Bound<'py, PyAny>,
}

/// Create or render a minimal SQLBuild project from a dbt project.
#[pyfunction]
#[pyo3(signature = (*, request))]
pub(crate) fn build_dbt_init_project<'py>(
    request: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = request.py();
    let cwd: Bound<'py, PyAny> = native_field(request, "cwd")?;
    let paths: InitPaths<'py> = init_paths(request)?;
    let sqb_output_dir: Bound<'py, PyAny> = native_field(request, "sqb_output_dir")?;
    let output_dir: Bound<'py, PyAny> = if sqb_output_dir.is_none() {
        joined(
            &native_field(&paths.dbt_project_dir, "parent")?,
            DEFAULT_OUTPUT_DIR,
        )?
    } else {
        resolved(&cwd, &sqb_output_dir)?
    };
    let project_file: Bound<'py, PyAny> = joined(&output_dir, PROJECT_FILE)?;
    if truthy(&project_file, "exists")? && !native_field(request, "overwrite")?.is_truthy()? {
        return Err(profile_error(
            py,
            format!(
                "{} already exists. Pass --overwrite to replace it.",
                project_file.str()?
            ),
        ));
    }
    if truthy(&output_dir, "exists")? && !truthy(&output_dir, "is_dir")? {
        return Err(profile_error(
            py,
            format!(
                "SQLBuild output path is not a directory: {}",
                output_dir.str()?
            ),
        ));
    }
    let callbacks: Bound<'py, PyAny> = native_field(request, "progress_callbacks")?;
    progress(&callbacks, "start", "Inspecting dbt project and profile...")?;
    let metadata: DbtProjectMetadata = load_dbt_project_metadata(&paths.dbt_project_dir)?;
    let profile_name: Bound<'py, PyAny> = profile_name(request, &metadata)?;
    let selected: Bound<'py, PyAny> = selected_output(&paths, request, &profile_name)?;
    progress(&callbacks, "complete", "Inspected dbt project and profile.")?;
    progress(&callbacks, "start", "Rendering dbt profile connection...")?;
    let resolved_output: Bound<'py, PyAny> = call_with(
        &host_entry(py, ThirdPartyHost::RenderSelectedDbtProfileOutput)?,
        &[
            ("selected", selected.clone()),
            ("project_dir", paths.dbt_project_dir.clone()),
            ("profiles_dir", paths.profiles_dir.clone()),
        ],
    )?;
    let normalized: Bound<'py, PyAny> = normalize_dbt_profile_connection(py, resolved_output)?;
    let target_path: Bound<'py, PyAny> = resolved(
        &paths.dbt_project_dir,
        &path_of(py, &PyString::new(py, &metadata.target_path).into_any())?,
    )?;
    let project_name: String = metadata.project_name.replace('-', "_");
    let target_name: Bound<'py, PyAny> = native_field(&selected, "target_name")?;
    let toml: Bound<'py, PyAny> = build_dbt_init_project_toml(
        py,
        Some(keywords(
            py,
            &[
                ("project_name", project_name.clone().into_bound_py_any(py)?),
                ("adapter", native_field(&normalized, "adapter")?),
                ("target_name", target_name.clone()),
                ("profile_name", profile_name.clone()),
                (
                    "dbt_project_dir",
                    display_path(&paths.dbt_project_dir, &output_dir)?,
                ),
                (
                    "profiles_dir",
                    display_path(&paths.profiles_dir, &output_dir)?,
                ),
                ("target_path", display_path(&target_path, &output_dir)?),
                ("target_schema", native_field(&normalized, "target_schema")?),
                (
                    "target_database",
                    native_field(&normalized, "target_database")?,
                ),
            ],
        )?),
    )
    .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
    progress(&callbacks, "complete", "Rendered dbt profile connection.")?;
    let mut warnings: Vec<Bound<'py, PyAny>> = native_field(&normalized, "warnings")?
        .try_iter()?
        .collect::<PyResult<_>>()?;
    if !native_field(request, "skip_dbt_debug")?.is_truthy()? {
        progress(&callbacks, "start", "Running dbt debug...")?;
        if let Some(warning) = dbt_debug_warning(&paths, &target_name.extract::<String>()?)? {
            warnings.push(PyString::new(py, &warning).into_any());
        }
        progress(&callbacks, "complete", "Finished dbt debug.")?;
    }
    let dry_run: Bound<'py, PyAny> = native_field(request, "dry_run")?;
    if !dry_run.is_truthy()? {
        progress(&callbacks, "start", "Writing SQLBuild project config...")?;
        let keywords: Bound<'py, PyDict> = PyDict::new(py);
        keywords.set_item("parents", true)?;
        keywords.set_item("exist_ok", true)?;
        output_dir.call_method("mkdir", (), Some(&keywords))?;
        let encoding: Bound<'py, PyDict> = PyDict::new(py);
        encoding.set_item("encoding", "utf-8")?;
        project_file.call_method("write_text", (&toml,), Some(&encoding))?;
        progress(&callbacks, "complete", "Wrote SQLBuild project config.")?;
    }
    new_record_with(
        py,
        RecordId::DbtInitResult,
        &[
            ("output_dir", output_dir),
            ("project_file", project_file),
            ("project_name", project_name.into_bound_py_any(py)?),
            ("adapter", native_field(&normalized, "adapter")?),
            ("target_name", target_name),
            ("profile_name", profile_name),
            ("toml", toml),
            ("warnings", PyTuple::new(py, warnings)?.into_any()),
            ("dry_run", dry_run),
        ],
    )
}

/// Validate dbt init project and profile discovery before interactive prompting.
#[pyfunction]
#[pyo3(signature = (*, request))]
pub(crate) fn validate_dbt_init_project(request: &Bound<'_, PyAny>) -> Result<(), PyErr> {
    let paths: InitPaths<'_> = init_paths(request)?;
    let metadata: DbtProjectMetadata = load_dbt_project_metadata(&paths.dbt_project_dir)?;
    let profile_name: Bound<'_, PyAny> = profile_name(request, &metadata)?;
    selected_output(&paths, request, &profile_name)?;
    Ok(())
}

fn init_paths<'py>(request: &Bound<'py, PyAny>) -> PyResult<InitPaths<'py>> {
    let cwd: Bound<'py, PyAny> = native_field(request, "cwd")?;
    let dbt_project_dir: Bound<'py, PyAny> =
        resolved(&cwd, &native_field(request, "dbt_project_dir")?)?;
    let profiles_dir: Bound<'py, PyAny> = profiles_dir(
        &cwd,
        &dbt_project_dir,
        &native_field(request, "profiles_dir")?,
    )?;
    Ok(InitPaths {
        dbt_project_dir,
        profiles_dir,
    })
}

/// `request.profile_name or metadata.profile_name`.
fn profile_name<'py>(
    request: &Bound<'py, PyAny>,
    metadata: &DbtProjectMetadata,
) -> PyResult<Bound<'py, PyAny>> {
    let requested: Bound<'py, PyAny> = native_field(request, "profile_name")?;
    if requested.is_truthy()? {
        return Ok(requested);
    }
    Ok(PyString::new(request.py(), &metadata.profile_name).into_any())
}

fn selected_output<'py>(
    paths: &InitPaths<'py>,
    request: &Bound<'py, PyAny>,
    profile_name: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let target_name: Option<String> = native_field(request, "target_name")?.extract()?;
    load_selected_dbt_profile_output(
        &paths.profiles_dir,
        &profile_name.extract::<String>()?,
        target_name.as_deref(),
    )
}

/// `path.expanduser()` resolved, relative paths below `root`.
fn resolved<'py>(
    root: &Bound<'py, PyAny>,
    path: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let expanded: Bound<'py, PyAny> = path.call_method0("expanduser")?;
    if truthy(&expanded, "is_absolute")? {
        return expanded.call_method0("resolve");
    }
    root.call_method1("__truediv__", (expanded,))?
        .call_method0("resolve")
}

/// The explicit, `DBT_PROFILES_DIR`, project, working or default `~/.dbt` profiles folder.
fn profiles_dir<'py>(
    cwd: &Bound<'py, PyAny>,
    dbt_project_dir: &Bound<'py, PyAny>,
    profiles_dir: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = cwd.py();
    if !profiles_dir.is_none() {
        return resolved(cwd, profiles_dir);
    }
    if let Some(value) = std::env::var_os(PROFILES_DIR_ENV_VAR).filter(|value| !value.is_empty()) {
        return resolved(cwd, &path_of(py, &value.into_bound_py_any(py)?)?);
    }
    if truthy(&joined(dbt_project_dir, PROFILES_FILE)?, "exists")? {
        return Ok(dbt_project_dir.clone());
    }
    if truthy(&joined(cwd, PROFILES_FILE)?, "exists")? {
        return cwd.call_method0("resolve");
    }
    path_of(py, &PyString::new(py, "~/.dbt").into_any())?.call_method0("expanduser")
}

/// The path as the generated config writes it: relative to `root`, else below home, else absolute.
fn display_path<'py>(
    path: &Bound<'py, PyAny>,
    root: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = path.py();
    match imported(py, "os.path", "relpath")?.call1((path, root)) {
        Ok(relative) => return path_of(py, &relative)?.call_method0("as_posix"),
        Err(error) if is_value_error(py, &error) => {}
        Err(error) => return Err(error),
    }
    match path.call_method1("relative_to", (root,)) {
        Ok(relative) => return relative.call_method0("as_posix"),
        Err(error) if is_value_error(py, &error) => {}
        Err(error) => return Err(error),
    }
    let home: Bound<'py, PyAny> = imported(py, "pathlib", "Path")?
        .call_method0("home")?
        .call_method0("resolve")?;
    match path.call_method1("relative_to", (home,)) {
        Ok(relative) => format!(
            "~/{}",
            relative.call_method0("as_posix")?.extract::<String>()?
        )
        .into_bound_py_any(py),
        Err(error) if is_value_error(py, &error) => path.call_method0("as_posix"),
        Err(error) => Err(error),
    }
}

/// The warning `dbt debug` adds, or `None` when it passed.
fn dbt_debug_warning(paths: &InitPaths<'_>, target_name: &str) -> PyResult<Option<String>> {
    let py: Python<'_> = paths.dbt_project_dir.py();
    let argv: Vec<String> = vec![
        "dbt".to_owned(),
        "debug".to_owned(),
        "--project-dir".to_owned(),
        paths.dbt_project_dir.call_method0("as_posix")?.extract()?,
        "--profiles-dir".to_owned(),
        paths.profiles_dir.call_method0("as_posix")?.extract()?,
        "--target".to_owned(),
        target_name.to_owned(),
    ];
    let output: io::Result<DbtProcessOutput> = py
        .compiler_detach(|| Ok(run_dbt_process(&argv, None)))
        .map_err(PyRuntimeError::new_err)?;
    py.check_signals()?;
    let output: DbtProcessOutput = match output {
        Ok(output) => output,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(Some(DEBUG_SKIPPED.to_owned()));
        }
        Err(error) => return Err(launch_os_error(py, &error, (&argv, None))),
    };
    if output.returncode == 0 {
        return Ok(None);
    }
    let detail: &str = python_strip(if output.stderr.is_empty() {
        &output.stdout
    } else {
        &output.stderr
    });
    if detail.is_empty() {
        return Ok(Some(DEBUG_FAILED.to_owned()));
    }
    Ok(Some(format!("{DEBUG_FAILED} {detail}")))
}

fn progress(callbacks: &Bound<'_, PyAny>, name: &str, message: &str) -> PyResult<()> {
    let callback: Bound<'_, PyAny> = native_field(callbacks, name)?;
    if !callback.is_none() {
        callback.call1((message,))?;
    }
    Ok(())
}

fn joined<'py>(directory: &Bound<'py, PyAny>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    directory.call_method1("__truediv__", (name,))
}

fn truthy(path: &Bound<'_, PyAny>, method: &str) -> PyResult<bool> {
    path.call_method0(method)?.is_truthy()
}

fn path_of<'py>(py: Python<'py>, value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    imported(py, "pathlib", "Path")?.call1((value,))
}

fn is_value_error(py: Python<'_>, error: &PyErr) -> bool {
    error.is_instance_of::<pyo3::exceptions::PyValueError>(py)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_dbt_init_project, module)?)?;
    module.add_function(wrap_pyfunction!(validate_dbt_init_project, module)?)
}
