//! Register the session functions and classes.

use crate::analysis_session::_helpers::session;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the session functions and classes.
#[inline]
pub fn register_session(module: &Bound<'_, PyModule>) -> PyResult<()> {
    session::register(module)
}
