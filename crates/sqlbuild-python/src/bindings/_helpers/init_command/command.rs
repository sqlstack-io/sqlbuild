//! `sqb init`: scaffold a blank SQLBuild project and its agent skills.

use sqlbuild_python_base::records::main::construct::new_error::new_error;
use std::path::PathBuf;

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::PyString;
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_core::constants::PROJECT_CONFIG_FILENAME as PROJECT_FILE;
use sqlbuild_core::init_command::main::init_project_name::init_project_name;
use sqlbuild_core::init_command::main::init_project_toml::init_project_toml;
use sqlbuild_core::init_command::main::render_init_summary::render_init_summary;

use crate::bindings::_helpers::init_command::scaffold::scaffold_blank_project;
use crate::bindings::_helpers::workspace_commands::commands::update_sqlbuild_skills;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::presentation_text::main::stdout_supports_color::stdout_supports_color;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;

const EXISTS_MESSAGE: &str = "sqlbuild_project.toml already exists in this directory";

fn exists_error(py: Python<'_>) -> PyErr {
    new_error(py, ErrorId::CliUser, EXISTS_MESSAGE, (Some("C900"), None))
}

/// Create a minimal SQLBuild project in the project directory.
#[pyfunction]
#[pyo3(signature = (project_dir = None))]
pub(crate) fn run_init_command(
    py: Python<'_>,
    project_dir: Option<Bound<'_, PyAny>>,
) -> PyResult<i32> {
    let base_dir: Bound<'_, PyAny> = match project_dir.filter(|path| !path.is_none()) {
        Some(path) => path,
        None => imported(py, "pathlib", "Path")?.call_method0("cwd")?,
    };
    if base_dir
        .call_method1("__truediv__", (PROJECT_FILE,))?
        .call_method0("exists")?
        .is_truthy()?
    {
        return Err(exists_error(py));
    }
    let project_name: String =
        init_project_name(&native_field(&base_dir, "name")?.extract::<String>()?);
    let base_path: PathBuf = base_dir.extract()?;
    scaffold_blank_project(py, &base_path, &init_project_toml(&project_name))?;
    update_sqlbuild_skills(py, Some(keywords(py, &[("project_dir", base_dir)])?))?;
    let style: CliStyle = CliStyle {
        use_color: stdout_supports_color(py)?,
    };
    py.import("sys")?.getattr("stdout")?.call_method1(
        "write",
        (PyString::new(
            py,
            &render_init_summary(&project_name, style),
        ),),
    )?;
    Ok(0)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(run_init_command, module)?)?;
    Ok(())
}
