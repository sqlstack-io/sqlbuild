//! Logical SQL refs outside comments and quotes, reporting rejected calls as P012.

use crate::sqltext::_helpers::sql_references;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyTuple;

/// Logical SQL refs outside comments and quotes, reporting rejected calls as P012.
#[inline]
pub fn extract_references<'py>(
    sql: &str,
    syntax: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    sql_references::extract_references(sql, syntax)
}
