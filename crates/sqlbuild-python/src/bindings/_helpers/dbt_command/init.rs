//! `sqb dbt init`: create or preview a SQLBuild twin project for a dbt project.

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_core::dbt_command::constants::DBT_PROJECT_FILE;
use sqlbuild_core::dbt_command::main::render_dbt_init_preview::render_dbt_init_preview;
use sqlbuild_core::dbt_command::main::render_dbt_init_summary::render_dbt_init_summary;
use sqlbuild_core::dbt_command::models::DbtInitSummary;
use sqlbuild_python_base::records::main::construct::new_error::new_error;

use crate::bindings::_helpers::dbt_command::init_project::build_dbt_init_project;
use crate::bindings::_helpers::dbt_command::init_project::validate_dbt_init_project;
use crate::bindings::_helpers::dbt_command::support::{color_enabled, flag, path, stream};
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_output::build_progress::models::DbtInitProgressReporter;

fn missing_project(py: Python<'_>) -> PyErr {
    new_error(
        py,
        ErrorId::CliUser,
        "sqb dbt init requires --project-dir pointing at a dbt project",
        (Some("C242"), None),
    )
}

/// `Path(value)`, or `None` when the option was not given.
fn optional_path<'py>(value: Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if value.is_none() {
        return Ok(value);
    }
    imported(value.py(), "pathlib", "Path")?.call1((value,))
}

/// The dbt project directory: the option, else `.` when the working directory is a dbt project.
fn dbt_project_dir<'py>(request: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = request.py();
    let given: Bound<'py, PyAny> = native_field(request, "dbt_project_dir")?;
    if !given.is_none() {
        return optional_path(given);
    }
    let in_project: bool = native_field(request, "cwd")?
        .call_method1("__truediv__", (DBT_PROJECT_FILE,))?
        .call_method0("exists")?
        .is_truthy()?;
    if in_project {
        return path(py, ".");
    }
    Err(missing_project(py))
}

fn text(result: &Bound<'_, PyAny>, name: &str) -> PyResult<String> {
    Ok(native_field(result, name)?.str()?.to_string())
}

fn summary(result: &Bound<'_, PyAny>) -> PyResult<DbtInitSummary> {
    Ok(DbtInitSummary {
        output_dir: text(result, "output_dir")?,
        project_file: text(result, "project_file")?,
        project_name: text(result, "project_name")?,
        adapter: text(result, "adapter")?,
        target_name: text(result, "target_name")?,
        profile_name: text(result, "profile_name")?,
        toml: text(result, "toml")?,
    })
}

/// Execute SQLBuild-owned `sqb dbt init`.
#[pyfunction]
#[pyo3(signature = (request))]
pub(crate) fn run_dbt_init_command(py: Python<'_>, request: &Bound<'_, PyAny>) -> PyResult<i32> {
    let project_dir: Bound<'_, PyAny> = dbt_project_dir(request)?;
    let use_color: bool = color_enabled(py)?;
    let progress: Bound<'_, PyAny> = DbtInitProgressReporter::new(
        &stream(py, "stdout")?,
        flag(py, use_color).extract::<bool>()?,
    )
    .and_then(|value| pyo3::Bound::new(py, value))
    .map(pyo3::Bound::into_any)?;
    let callbacks: Bound<'_, PyAny> = new_record_with(
        py,
        RecordId::DbtInitProgressCallbacks,
        &[
            ("start", native_field(&progress, "start")?),
            ("complete", native_field(&progress, "complete")?),
        ],
    )?;
    let init_request: Bound<'_, PyAny> = new_record_with(
        py,
        RecordId::DbtInitRequest,
        &[
            ("cwd", native_field(request, "cwd")?),
            ("dbt_project_dir", project_dir),
            (
                "profiles_dir",
                optional_path(native_field(request, "profiles_dir")?)?,
            ),
            ("profile_name", native_field(request, "profile_name")?),
            ("target_name", native_field(request, "target_name")?),
            (
                "sqb_output_dir",
                optional_path(native_field(request, "sqb_output_dir")?)?,
            ),
            ("dry_run", native_field(request, "dry_run")?),
            ("overwrite", native_field(request, "overwrite")?),
            ("skip_dbt_debug", native_field(request, "skip_dbt_debug")?),
            ("progress_callbacks", callbacks),
        ],
    )?;
    validate_dbt_init_project(&init_request.clone())
        .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
    let result: Bound<'_, PyAny> = build_dbt_init_project(&init_request)?;
    let style: CliStyle = CliStyle { use_color };
    let stdout: Bound<'_, PyAny> = stream(py, "stdout")?;
    if native_field(request, "dry_run")?.is_truthy()? {
        stdout.call_method1(
            "write",
            (render_dbt_init_preview(&summary(&result)?, style),),
        )?;
        return Ok(0);
    }
    stdout.call_method1(
        "write",
        (render_dbt_init_summary(&summary(&result)?, style),),
    )?;
    for warning in native_field(&result, "warnings")?.try_iter()? {
        stream(py, "stderr")?
            .call_method1("write", (format!("Warning: {}\n", warning?.str()?),))?;
    }
    Ok(0)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(run_dbt_init_command, module)?)?;
    Ok(())
}
