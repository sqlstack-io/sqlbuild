//! Register the fitting functions and classes.

use crate::relation_naming::_helpers::fitting;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the fitting functions and classes.
#[inline]
pub fn register_fitting(module: &Bound<'_, PyModule>) -> PyResult<()> {
    fitting::register(module)
}
