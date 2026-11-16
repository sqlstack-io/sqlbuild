//! Resource assembly facts for one project, computed natively.

use crate::project_assembly::main::syntax_error_message::syntax_error_message;
use std::collections::HashMap;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyResult, Python};
use pyo3::types::{PyString, PyStringMethods};
use pyo3::{PyErr, pyfunction};
use sqlbuild_analysis::assembly::project::models::{SyntaxCheck, SyntaxFailure, SyntaxMode};

pub(crate) const SYNTAX_CHECK_CONTEXT: &str = "native SQL syntax check";

/// Python's syntax error message for one SQL string, None where it parses.
#[pyfunction]
#[pyo3(signature = (sql, placeholders, dialect, parse_one=false))]
pub(crate) fn sql_syntax_error(
    sql: &Bound<'_, PyString>,
    placeholders: Option<HashMap<String, String>>,
    dialect: Option<String>,
    parse_one: bool,
) -> PyResult<Option<String>> {
    let py: Python<'_> = sql.py();
    let check = SyntaxCheck {
        sql: sql.to_str()?.to_owned(),
        placeholders: placeholders.unwrap_or_default().into_iter().collect(),
    };
    let dialect: String = dialect
        .filter(|dialect| !dialect.is_empty())
        .unwrap_or_else(|| "generic".to_owned());
    let mode = if parse_one {
        SyntaxMode::Parse
    } else {
        SyntaxMode::Validate
    };
    syntax_error_message(py, &check, &dialect, mode)
}

/// The `ValueError` Python's syntax validation raises before parsing.
pub(crate) fn syntax_value_error(failure: SyntaxFailure) -> PyErr {
    match failure {
        SyntaxFailure::Normalization(message) => PyValueError::new_err(message),
        SyntaxFailure::UnknownDialect(name) => {
            PyValueError::new_err(format!("Unknown dialect: {name}"))
        }
    }
}
