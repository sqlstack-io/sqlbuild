//! Register the key checks functions and classes.

use crate::discovery::_helpers::headers::key_checks;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the key checks functions and classes.
#[inline]
pub fn register_key_checks(module: &Bound<'_, PyModule>) -> PyResult<()> {
    key_checks::register(module)
}
