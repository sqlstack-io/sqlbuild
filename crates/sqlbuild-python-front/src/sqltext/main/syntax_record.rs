//! A `SqlLexicalSyntax` record holding `syntax`.

use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::PyFrozenSet;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

/// A `SqlLexicalSyntax` record holding `syntax`.
pub fn syntax_record<'py>(py: Python<'py>, syntax: &LexicalSyntax) -> PyResult<Bound<'py, PyAny>> {
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
