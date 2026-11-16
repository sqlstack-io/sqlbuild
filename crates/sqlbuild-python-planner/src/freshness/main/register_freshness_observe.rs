//! Register the freshness observe functions and classes.

use crate::freshness::_helpers::freshness_observe;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the freshness observe functions and classes.
#[inline]
pub fn register_freshness_observe(module: &Bound<'_, PyModule>) -> PyResult<()> {
    freshness_observe::register(module)
}
