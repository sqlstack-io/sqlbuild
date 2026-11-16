//! `load_project_config`: the validated `ProjectConfig` of `project_dir`.

use crate::discovery::_helpers::project::project_config::built;
use crate::discovery::_helpers::project::project_config::files;
use crate::discovery::_helpers::project::project_config::load_error;
use pyo3::prelude::{Bound, Py, PyAny, PyResult, Python};
use pyo3::pyfunction;
use sqlbuild_config::project::main::load_project_config::load_project_config;

/// `load_project_config`: the validated `ProjectConfig` of `project_dir`.
#[pyfunction]
pub fn load_project_config_file(
    py: Python<'_>,
    project_dir: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    let files = files(project_dir)?;
    match load_project_config(&files.native) {
        Ok(config) => built(py, config),
        Err(failure) => Err(load_error(py, &failure, &files)?),
    }
}
