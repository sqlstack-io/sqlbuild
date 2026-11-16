//! Validate that a model's query parses, raising its syntax error.

use crate::project_assembly::_helpers::syntax_validation::GENERIC_DIALECT;
use crate::project_assembly::_helpers::syntax_validation::syntax_error;
use crate::project_assembly::main::syntax_error_message::syntax_error_message;
use crate::project_assembly::types::ParseOptions;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, Python};
use pyo3::pyfunction;
use sqlbuild_analysis::assembly::project::models::{SyntaxCheck, SyntaxMode};

/// Validate that a model's query parses, raising its syntax error.
#[pyfunction]
#[pyo3(signature = (query_sql, model, parsing=(None, None)))]
pub fn validate_sql_syntax(
    py: Python<'_>,
    query_sql: &str,
    model: (String, Bound<'_, PyAny>),
    parsing: ParseOptions,
) -> Result<(), PyErr> {
    let (model_name, file_path) = model;
    let (placeholders, dialect) = parsing;
    let check: SyntaxCheck = SyntaxCheck {
        sql: query_sql.to_owned(),
        placeholders: placeholders.unwrap_or_default().into_iter().collect(),
    };
    let dialect: String = dialect
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| GENERIC_DIALECT.to_owned());
    match syntax_error_message(py, &check, &dialect, SyntaxMode::Validate)? {
        Some(message) => Err(syntax_error(
            py,
            format!(
                "SQL syntax error in model '{model_name}' ({}): {message}",
                file_path.str()?
            ),
        )),
        None => Ok(()),
    }
}
