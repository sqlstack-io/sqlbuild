//! Register the guidance functions and classes.

use crate::discovery::_helpers::failures::guidance;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the guidance functions and classes.
#[inline]
pub fn register_guidance(module: &Bound<'_, PyModule>) -> PyResult<()> {
    guidance::register(module)
}
