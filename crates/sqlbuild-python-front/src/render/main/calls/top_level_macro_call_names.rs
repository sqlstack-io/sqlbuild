//! Python's `find_macro_call_names`: top-level call names in encounter order, declarations excluded.

use crate::render::_helpers::calls::macro_calls;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyTuple;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

/// Python's `find_macro_call_names`: top-level call names in encounter order, declarations excluded.
#[inline]
pub fn top_level_macro_call_names<'py>(
    py: Python<'py>,
    sql: &str,
    syntax: Option<&LexicalSyntax>,
) -> PyResult<Bound<'py, PyTuple>> {
    macro_calls::top_level_macro_call_names(py, sql, syntax)
}
