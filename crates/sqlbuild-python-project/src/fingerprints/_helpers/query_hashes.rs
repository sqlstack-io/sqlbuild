//! Query and function-definition change fingerprints.

use pyo3::prelude::{PyAnyMethods, PyErr, PyResult, Python};
use pyo3::pyfunction;
use sqlbuild_analysis::sql_tokens::main::definition_fingerprint::definition_fingerprint;
use sqlbuild_analysis::sql_tokens::main::normalized_query_text::normalized_query_text;

use sqlbuild_core::panics::main::is_compiler_panic::is_compiler_panic;

use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::types::CompilerDetach;

/// `QueryFingerprintError(message)`, or the compiler error a native panic becomes.
pub(crate) fn query_fingerprint_error(py: Python<'_>, message: String) -> PyErr {
    if is_compiler_panic(&message) {
        return compiler_error(message);
    }
    match error_class(py, ErrorId::QueryFingerprint).and_then(|class| class.call1((message,))) {
        Ok(error) => PyErr::from_value(error),
        Err(error) => error,
    }
}

/// A function definition's fingerprint: SQL bodies like queries, other languages exactly.
#[pyfunction]
#[pyo3(signature = (*, fingerprint_sql, language, dialect))]
pub(crate) fn compute_function_definition_hash(
    py: Python<'_>,
    fingerprint_sql: &str,
    language: &str,
    dialect: Option<&str>,
) -> PyResult<String> {
    py.compiler_detach(|| definition_fingerprint(fingerprint_sql, language, dialect))
        .map_err(|error| query_fingerprint_error(py, error))
}

/// Query SQL stripped with its whitespace runs collapsed.
#[pyfunction]
pub(crate) fn normalize_query_sql(query_sql: &str) -> String {
    normalized_query_text(query_sql)
}
