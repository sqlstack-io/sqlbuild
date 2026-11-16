//! A `SqlLexicalSyntax` record holding `syntax`.

use crate::sqltext::_helpers::lexical_syntax;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

/// A `SqlLexicalSyntax` record holding `syntax`.
#[inline]
pub fn syntax_record<'py>(py: Python<'py>, syntax: &LexicalSyntax) -> PyResult<Bound<'py, PyAny>> {
    lexical_syntax::syntax_record(py, syntax)
}
