//! Register the freshness functions and classes.

use crate::state_rows::_helpers::events::freshness;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the freshness functions and classes.
#[inline]
pub fn register_freshness(module: &Bound<'_, PyModule>) -> PyResult<()> {
    freshness::register(module)
}
