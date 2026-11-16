//! The persisted SHA-256 fingerprint of the query's canonical token stream.

use crate::fingerprints::_helpers::query_hashes;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// The persisted SHA-256 fingerprint of the query's canonical token stream.
#[inline]
pub fn compute_query_hash(
    py: Python<'_>,
    query_sql: &str,
    dialect: Option<&str>,
) -> PyResult<String> {
    query_hashes::compute_query_hash(py, query_sql, dialect)
}
