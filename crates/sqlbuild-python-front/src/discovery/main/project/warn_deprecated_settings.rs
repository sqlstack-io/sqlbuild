//! Write a warning line to stderr for each deprecated setting alias `project_dir` still sets.

use crate::discovery::_helpers::project::project_config;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// Write a warning line to stderr for each deprecated setting alias `project_dir` still sets.
#[inline]
pub fn warn_deprecated_settings(py: Python<'_>, project_dir: &Bound<'_, PyAny>) -> PyResult<()> {
    project_config::warn_deprecated_settings(py, project_dir)
}
