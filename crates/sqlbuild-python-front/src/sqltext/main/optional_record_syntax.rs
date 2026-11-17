//! The lexical rules a `SqlLexicalSyntax` record holds, or `None` for `None`.

use crate::sqltext::main::record_syntax::record_syntax;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

/// The lexical rules a `SqlLexicalSyntax` record holds, or `None` for `None`.
pub fn optional_record_syntax(value: &Bound<'_, PyAny>) -> PyResult<Option<LexicalSyntax>> {
    if value.is_none() {
        return Ok(None);
    }
    record_syntax(value).map(Some)
}
