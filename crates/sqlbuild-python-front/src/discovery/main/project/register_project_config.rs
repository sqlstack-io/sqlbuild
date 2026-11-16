//! Register the project config functions and classes.

use crate::discovery::_helpers::project::project_config;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the project config functions and classes.
#[inline]
pub fn register_project_config(module: &Bound<'_, PyModule>) -> PyResult<()> {
    project_config::register(module)
}
