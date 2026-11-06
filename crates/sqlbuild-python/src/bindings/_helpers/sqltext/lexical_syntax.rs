//! One adapter's `SqlLexicalSyntax`, read from a Python mapping.

use pyo3::FromPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyFrozenSet;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

use crate::bindings::_helpers::records::construct::new_record_with;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::RecordId;

/// One adapter's `SqlLexicalSyntax`, read from a Python mapping.
#[derive(Clone, FromPyObject)]
#[pyo3(from_item_all)]
pub(crate) struct LexicalSyntaxInput {
    backslash_escape_quotes: Vec<String>,
    escape_string_prefix: bool,
    raw_string_prefix: bool,
    triple_quoted_strings: bool,
    nested_block_comments: bool,
    line_comment_prefixes: Vec<String>,
}

impl From<LexicalSyntaxInput> for LexicalSyntax {
    fn from(input: LexicalSyntaxInput) -> Self {
        Self {
            backslash_escape_quotes: input.backslash_escape_quotes,
            escape_string_prefix: input.escape_string_prefix,
            raw_string_prefix: input.raw_string_prefix,
            triple_quoted_strings: input.triple_quoted_strings,
            nested_block_comments: input.nested_block_comments,
            line_comment_prefixes: input.line_comment_prefixes,
        }
    }
}

/// The sorted texts of a set of strings.
pub(crate) fn sorted_texts(values: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    let mut texts: Vec<String> = values
        .try_iter()?
        .map(|value| value?.extract::<String>())
        .collect::<PyResult<_>>()?;
    texts.sort_unstable();
    Ok(texts)
}

/// The lexical rules a `SqlLexicalSyntax` record holds.
pub(crate) fn record_syntax(instance: &Bound<'_, PyAny>) -> PyResult<LexicalSyntax> {
    let flag = |name: &str| -> PyResult<bool> { native_field(instance, name)?.is_truthy() };
    Ok(LexicalSyntax {
        backslash_escape_quotes: sorted_texts(&native_field(instance, "backslash_escape_quotes")?)?,
        escape_string_prefix: flag("escape_string_prefix")?,
        raw_string_prefix: flag("raw_string_prefix")?,
        triple_quoted_strings: flag("triple_quoted_strings")?,
        nested_block_comments: flag("nested_block_comments")?,
        line_comment_prefixes: sorted_texts(&native_field(instance, "line_comment_prefixes")?)?,
    })
}

/// The lexical rules a `SqlLexicalSyntax` record holds, or `None` for `None`.
pub(crate) fn optional_record_syntax(value: &Bound<'_, PyAny>) -> PyResult<Option<LexicalSyntax>> {
    if value.is_none() {
        return Ok(None);
    }
    record_syntax(value).map(Some)
}

/// A `SqlLexicalSyntax` record holding `syntax`.
pub(crate) fn syntax_record<'py>(
    py: Python<'py>,
    syntax: &LexicalSyntax,
) -> PyResult<Bound<'py, PyAny>> {
    let flag = |value: bool| -> PyResult<Bound<'py, PyAny>> {
        Ok(pyo3::types::PyBool::new(py, value).to_owned().into_any())
    };
    new_record_with(
        py,
        RecordId::SqlLexicalSyntax,
        &[
            (
                "backslash_escape_quotes",
                PyFrozenSet::new(py, &syntax.backslash_escape_quotes)?.into_any(),
            ),
            ("escape_string_prefix", flag(syntax.escape_string_prefix)?),
            ("raw_string_prefix", flag(syntax.raw_string_prefix)?),
            ("triple_quoted_strings", flag(syntax.triple_quoted_strings)?),
            ("nested_block_comments", flag(syntax.nested_block_comments)?),
            (
                "line_comment_prefixes",
                PyFrozenSet::new(py, &syntax.line_comment_prefixes)?.into_any(),
            ),
        ],
    )
}
