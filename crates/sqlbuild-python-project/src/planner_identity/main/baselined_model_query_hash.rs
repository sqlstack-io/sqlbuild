//! The model's query fingerprint, or its recorded one while the recorded SQL still fingerprints the same.

use crate::planner_identity::_helpers::hashing;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// The model's query fingerprint, or its recorded one while the recorded SQL still fingerprints the same.
#[inline]
pub fn baselined_model_query_hash(
    py: Python<'_>,
    model_name: &str,
    (query_sql, dialect): (&str, Option<&str>),
    recorded: &Bound<'_, PyAny>,
) -> PyResult<String> {
    hashing::baselined_model_query_hash(py, model_name, (query_sql, dialect), recorded)
}
