//! The lexical rules a `SqlLexicalSyntax` record holds.

use crate::sqltext::main::sorted_texts::sorted_texts;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

/// The lexical rules a `SqlLexicalSyntax` record holds.
pub fn record_syntax(instance: &Bound<'_, PyAny>) -> PyResult<LexicalSyntax> {
    let flag = |name: &str| -> PyResult<bool> { native_field(instance, name)?.is_truthy() };
    Ok(LexicalSyntax {
        backslash_escape_quotes: sorted_texts(&native_field(instance, "backslash_escape_quotes")?)?,
        escape_string_prefix: flag("escape_string_prefix")?,
        raw_string_prefix: flag("raw_string_prefix")?,
        triple_quoted_strings: flag("triple_quoted_strings")?,
        nested_block_comments: flag("nested_block_comments")?,
        line_comment_prefixes: sorted_texts(&native_field(instance, "line_comment_prefixes")?)?,
        bracket_identifiers: false,
    })
}
