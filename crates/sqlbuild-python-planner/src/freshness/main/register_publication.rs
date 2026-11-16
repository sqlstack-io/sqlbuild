//! Register the publication functions and classes.

use crate::freshness::_helpers::publication;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the publication functions and classes.
#[inline]
pub fn register_publication(module: &Bound<'_, PyModule>) -> PyResult<()> {
    publication::register(module)
}
