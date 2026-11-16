//! The project or local configuration file path, rejecting an unconverted legacy file.

use crate::discovery::_helpers::project::project_config;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// The project or local configuration file path, rejecting an unconverted legacy file.
#[inline]
pub fn resolve_config_file_path<'py>(
    py: Python<'py>,
    project_dir: &Bound<'py, PyAny>,
    local: bool,
) -> PyResult<Bound<'py, PyAny>> {
    project_config::resolve_config_file_path(py, project_dir, local)
}
