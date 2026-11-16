//! The lexical rules a `SqlLexicalSyntax` record holds.

use crate::sqltext::_helpers::lexical_syntax;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

/// The lexical rules a `SqlLexicalSyntax` record holds.
#[inline]
pub fn record_syntax(instance: &Bound<'_, PyAny>) -> PyResult<LexicalSyntax> {
    lexical_syntax::record_syntax(instance)
}
