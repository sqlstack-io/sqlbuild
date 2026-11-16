//! Register the fixture columns functions and classes.

use crate::analysis_session::_helpers::fixture_columns;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the fixture columns functions and classes.
#[inline]
pub fn register_fixture_columns(module: &Bound<'_, PyModule>) -> PyResult<()> {
    fixture_columns::register(module)
}
