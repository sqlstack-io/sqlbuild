//! Register the query columns functions and classes.

use crate::analysis_session::_helpers::query_columns;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the query columns functions and classes.
#[inline]
pub fn register_query_columns(module: &Bound<'_, PyModule>) -> PyResult<()> {
    query_columns::register(module)
}
