//! Register the transient status functions and classes.

use crate::build_progress::_helpers::transient_status;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the transient status functions and classes.
#[inline]
pub fn register_transient_status(module: &Bound<'_, PyModule>) -> PyResult<()> {
    transient_status::register(module)
}
