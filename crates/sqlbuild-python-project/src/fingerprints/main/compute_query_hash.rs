//! The persisted SHA-256 fingerprint of the query's canonical token stream.

use crate::fingerprints::_helpers::query_hashes::query_fingerprint_error;
use pyo3::prelude::{PyResult, Python};
use pyo3::pyfunction;
use sqlbuild_analysis::sql_tokens::main::compiler_query_fingerprint::compiler_query_fingerprint;
use sqlbuild_python_base::types::CompilerDetach;

/// The persisted SHA-256 fingerprint of the query's canonical token stream.
#[pyfunction]
#[pyo3(signature = (*, query_sql, dialect))]
pub fn compute_query_hash(
    py: Python<'_>,
    query_sql: &str,
    dialect: Option<&str>,
) -> PyResult<String> {
    py.compiler_detach(|| compiler_query_fingerprint(query_sql, dialect))
        .map_err(|error| query_fingerprint_error(py, error))
}
