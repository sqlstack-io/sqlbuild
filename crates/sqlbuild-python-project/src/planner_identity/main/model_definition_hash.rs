//! Fingerprint a model query for change detection, naming the model when it cannot.

use crate::planner_identity::_helpers::hashing;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// Fingerprint a model query for change detection, naming the model when it cannot.
#[inline]
pub fn model_definition_hash(
    py: Python<'_>,
    model_name: &str,
    query_sql: &str,
    dialect: Option<&str>,
) -> PyResult<String> {
    hashing::model_definition_hash(py, model_name, query_sql, dialect)
}
