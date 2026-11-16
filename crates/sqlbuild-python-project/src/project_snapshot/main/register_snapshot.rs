//! Register the snapshot functions and classes.

use crate::project_snapshot::_helpers::snapshot::snapshot;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the snapshot functions and classes.
#[inline]
pub fn register_snapshot(module: &Bound<'_, PyModule>) -> PyResult<()> {
    snapshot::register(module)
}
