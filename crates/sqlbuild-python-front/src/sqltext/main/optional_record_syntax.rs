//! The lexical rules a `SqlLexicalSyntax` record holds, or `None` for `None`.

use crate::sqltext::_helpers::lexical_syntax;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

/// The lexical rules a `SqlLexicalSyntax` record holds, or `None` for `None`.
#[inline]
pub fn optional_record_syntax(value: &Bound<'_, PyAny>) -> PyResult<Option<LexicalSyntax>> {
    lexical_syntax::optional_record_syntax(value)
}
