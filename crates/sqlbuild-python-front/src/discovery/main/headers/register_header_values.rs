//! Register the header values functions and classes.

use crate::discovery::_helpers::headers::header_values;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the header values functions and classes.
#[inline]
pub fn register_header_values(module: &Bound<'_, PyModule>) -> PyResult<()> {
    header_values::register(module)
}
