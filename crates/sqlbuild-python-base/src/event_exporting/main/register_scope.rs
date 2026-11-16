//! Register the scope functions and classes.

use crate::event_exporting::_helpers::scope;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the scope functions and classes.
#[inline]
pub fn register_scope(module: &Bound<'_, PyModule>) -> PyResult<()> {
    scope::register(module)
}
