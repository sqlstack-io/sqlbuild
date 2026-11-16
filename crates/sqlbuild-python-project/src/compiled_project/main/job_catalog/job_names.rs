//! The command's job and job group names, loaded on first use; empty outside a command.

use crate::compiled_project::_helpers::job_catalog;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// The command's job and job group names, loaded on first use; empty outside a command.
#[inline]
pub fn job_names(py: Python<'_>) -> PyResult<Vec<String>> {
    job_catalog::job_names(py)
}
