//! `load_project_config`: the validated `ProjectConfig` of `project_dir`.

use crate::discovery::_helpers::project::project_config;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// `load_project_config`: the validated `ProjectConfig` of `project_dir`.
#[inline]
pub fn load_project_config_file(
    py: Python<'_>,
    project_dir: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    project_config::load_project_config_file(py, project_dir)
}
