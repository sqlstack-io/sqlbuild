//! Register the transient lines functions and classes.

use crate::build_progress::_helpers::transient_lines;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the transient lines functions and classes.
#[inline]
pub fn register_transient_lines(module: &Bound<'_, PyModule>) -> PyResult<()> {
    transient_lines::register(module)
}
