//! Register the relation keys functions and classes.

use crate::compiled_views::_helpers::relation_keys;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the relation keys functions and classes.
#[inline]
pub fn register_relation_keys(module: &Bound<'_, PyModule>) -> PyResult<()> {
    relation_keys::register(module)
}
