//! Register the nullability functions and classes.

use crate::type_system::_helpers::nullability;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the nullability functions and classes.
#[inline]
pub fn register_nullability(module: &Bound<'_, PyModule>) -> PyResult<()> {
    nullability::register(module)
}
