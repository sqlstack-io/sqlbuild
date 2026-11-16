//! `load_local_config`: the validated `LocalConfig` of `project_dir`, or the default.

use crate::discovery::_helpers::project::project_config::built;
use crate::discovery::_helpers::project::project_config::files;
use crate::discovery::_helpers::project::project_config::load_error;
use pyo3::prelude::{Bound, Py, PyAny, PyResult, Python};
use pyo3::pyfunction;
use sqlbuild_config::project::main::load_local_config::load_local_config;

/// `load_local_config`: the validated `LocalConfig` of `project_dir`, or the default.
#[pyfunction]
pub fn load_local_config_file(
    py: Python<'_>,
    project_dir: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    let files = files(project_dir)?;
    match load_local_config(&files.native) {
        Ok(config) => built(py, config),
        Err(failure) => Err(load_error(py, &failure, &files)?),
    }
}
