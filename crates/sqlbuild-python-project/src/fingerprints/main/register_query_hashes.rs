//! Register the query hashes functions and classes.

use crate::fingerprints::_helpers::query_hashes;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the query hashes functions and classes.
#[inline]
pub fn register_query_hashes(module: &Bound<'_, PyModule>) -> PyResult<()> {
    query_hashes::register(module)
}
