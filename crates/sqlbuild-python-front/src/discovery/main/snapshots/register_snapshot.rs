//! Register the snapshot functions and classes.

use crate::discovery::_helpers::snapshots::snapshot;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the snapshot functions and classes.
#[inline]
pub fn register_snapshot(module: &Bound<'_, PyModule>) -> PyResult<()> {
    snapshot::register(module)
}
