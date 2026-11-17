//! Register the fields functions and classes.

use crate::authored_values::_helpers::fields;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the fields functions and classes.
#[inline]
pub fn register_fields(module: &Bound<'_, PyModule>) -> PyResult<()> {
    fields::register(module)
}
