//! Register the project vars functions and classes.

use crate::authored_values::_helpers::project_vars;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the project vars functions and classes.
#[inline]
pub fn register_project_vars(module: &Bound<'_, PyModule>) -> PyResult<()> {
    project_vars::register(module)
}
