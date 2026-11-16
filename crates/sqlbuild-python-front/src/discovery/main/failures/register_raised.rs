//! Register the raised functions and classes.

use crate::discovery::_helpers::failures::raised;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the raised functions and classes.
#[inline]
pub fn register_raised(module: &Bound<'_, PyModule>) -> PyResult<()> {
    raised::register(module)
}
