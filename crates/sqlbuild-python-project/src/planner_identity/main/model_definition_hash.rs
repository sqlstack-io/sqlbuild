//! Fingerprint a model query for change detection, naming the model when it cannot.

use crate::planner_identity::_helpers::hashing::model_query_hash;
use pyo3::prelude::{PyResult, Python};
use pyo3::pyfunction;

/// Fingerprint a model query for change detection, naming the model when it cannot.
#[pyfunction]
#[pyo3(signature = (*, model_name, query_sql, dialect))]
pub fn model_definition_hash(
    py: Python<'_>,
    model_name: &str,
    query_sql: &str,
    dialect: Option<&str>,
) -> PyResult<String> {
    model_query_hash(py, model_name, (query_sql, dialect))
}
