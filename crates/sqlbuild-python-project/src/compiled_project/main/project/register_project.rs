//! Register the project functions and classes.

use crate::compiled_project::_helpers::project;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the project functions and classes.
#[inline]
pub fn register_project(module: &Bound<'_, PyModule>) -> PyResult<()> {
    project::register(module)
}
