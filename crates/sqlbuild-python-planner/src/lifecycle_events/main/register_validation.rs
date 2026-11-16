//! Register the validation functions and classes.

use crate::lifecycle_events::_helpers::validation;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the validation functions and classes.
#[inline]
pub fn register_validation(module: &Bound<'_, PyModule>) -> PyResult<()> {
    validation::register(module)
}
