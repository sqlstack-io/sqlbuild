//! Register the project validation functions and classes.

use crate::discovery::_helpers::project::project_validation;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the project validation functions and classes.
#[inline]
pub fn register_project_validation(module: &Bound<'_, PyModule>) -> PyResult<()> {
    project_validation::register(module)
}
