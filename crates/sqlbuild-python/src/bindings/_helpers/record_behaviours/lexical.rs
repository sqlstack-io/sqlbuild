//! Validation and derived views of the natively hosted `SqlLexicalSyntax` record.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyList, PyTuple, PyTupleMethods};

use sqlbuild_sqltext::sql_scan::constants::SUPPORTED_LINE_COMMENT_PREFIXES;
use sqlbuild_sqltext::sql_scan::main::lexical_syntax_key::lexical_syntax_key;
use sqlbuild_sqltext::sql_scan::main::reads_differently_from_generic::reads_differently_from_generic;
use sqlbuild_sqltext::sql_scan::main::unsupported_line_comment_prefixes::unsupported_line_comment_prefixes;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

use crate::bindings::_helpers::records::construct::error_class;
use crate::bindings::_helpers::records::ids::ErrorId;
use crate::bindings::_helpers::sqltext::lexical_syntax::{record_syntax, sorted_texts};

/// `SqlLexicalSyntax.line_comment_prefixes` position.
const LINE_COMMENT_PREFIXES_FIELD: usize = 5;

/// `SqlLexicalSyntax`: reject line comment prefixes the scanners cannot read.
pub(crate) fn finish_lexical_syntax(
    py: Python<'_>,
    values: Vec<Py<PyAny>>,
) -> PyResult<Vec<Py<PyAny>>> {
    let prefixes: Vec<String> = sorted_texts(values[LINE_COMMENT_PREFIXES_FIELD].bind(py))?;
    let unsupported: Vec<String> = unsupported_line_comment_prefixes(&prefixes);
    if unsupported.is_empty() {
        return Ok(values);
    }
    let supported: Vec<&str> = SUPPORTED_LINE_COMMENT_PREFIXES.to_vec();
    let message: String = format!(
        "SQL line comment prefixes {} are not supported; SQLBuild scans only {}",
        PyList::new(py, &unsupported)?.repr()?,
        PyList::new(py, &supported)?.repr()?
    );
    let error: Bound<'_, PyAny> =
        error_class(py, ErrorId::UnsupportedLexicalSyntax)?.call1((message,))?;
    Err(PyErr::from_value(error))
}

/// `SqlLexicalSyntax.cache_key`: a process-stable text key for these rules.
pub(crate) fn lexical_cache_key(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    lexical_syntax_key(&record_syntax(instance)?).into_py_any(instance.py())
}

/// `SqlLexicalSyntax.native_mapping`: these rules as the mapping native scanners read.
pub(crate) fn lexical_native_mapping(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let py: Python<'_> = instance.py();
    let syntax: LexicalSyntax = record_syntax(instance)?;
    let mapping: Bound<'_, PyDict> = PyDict::new(py);
    mapping.set_item(
        "backslash_escape_quotes",
        PyList::new(py, &syntax.backslash_escape_quotes)?,
    )?;
    mapping.set_item("escape_string_prefix", syntax.escape_string_prefix)?;
    mapping.set_item("raw_string_prefix", syntax.raw_string_prefix)?;
    mapping.set_item("triple_quoted_strings", syntax.triple_quoted_strings)?;
    mapping.set_item("nested_block_comments", syntax.nested_block_comments)?;
    mapping.set_item(
        "line_comment_prefixes",
        PyList::new(py, &syntax.line_comment_prefixes)?,
    )?;
    Ok(mapping.into_any().unbind())
}

/// `SqlLexicalSyntax.reads_differently_from_generic(sql)`.
pub(crate) fn lexical_reads_differently(
    instance: &Bound<'_, PyAny>,
    args: &Bound<'_, PyTuple>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    let keyword: Option<Bound<'_, PyAny>> = match kwargs {
        Some(kwargs) => kwargs.get_item("sql")?,
        None => None,
    };
    let sql: String = match keyword {
        Some(sql) => sql.extract()?,
        None => args.get_item(0)?.extract()?,
    };
    reads_differently_from_generic(&record_syntax(instance)?, &sql).into_py_any(instance.py())
}
