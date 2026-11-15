//! The interpreter running SQLBuild, which child processes reuse.

use crate::native_host::_helpers::host;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use std::path::PathBuf;

/// The interpreter running SQLBuild, which child processes reuse.
#[inline]
pub fn python_executable(py: Python<'_>) -> PyResult<PathBuf> {
    host::python_executable(py)
}
