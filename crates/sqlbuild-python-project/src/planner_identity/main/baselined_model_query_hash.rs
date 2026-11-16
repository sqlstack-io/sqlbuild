//! The model's query fingerprint, or its recorded one while the recorded SQL still fingerprints the same.

use crate::planner_identity::_helpers::hashing::model_query_hash;
use crate::planner_identity::_helpers::hashing::recorded_baseline;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_analysis::sql_tokens::main::compiler_query_fingerprint::compiler_query_fingerprint;

/// The model's query fingerprint, or its recorded one while the recorded SQL still fingerprints the same.
pub fn baselined_model_query_hash(
    py: Python<'_>,
    model_name: &str,
    (query_sql, dialect): (&str, Option<&str>),
    recorded: &Bound<'_, PyAny>,
) -> PyResult<String> {
    let current: String = model_query_hash(py, model_name, (query_sql, dialect))?;
    recorded_baseline(py, current, recorded, |sql| {
        compiler_query_fingerprint(sql, dialect)
    })
}
