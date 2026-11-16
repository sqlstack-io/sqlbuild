//! Register the progress functions and classes.

use crate::build_progress::_helpers::progress;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the progress functions and classes.
#[inline]
pub fn register_progress(module: &Bound<'_, PyModule>) -> PyResult<()> {
    progress::register(module)
}
