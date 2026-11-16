//! Register the scope snapshot functions and classes.

use crate::discovery::_helpers::snapshots::scope_snapshot;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the scope snapshot functions and classes.
#[inline]
pub fn register_scope_snapshot(module: &Bound<'_, PyModule>) -> PyResult<()> {
    scope_snapshot::register(module)
}
