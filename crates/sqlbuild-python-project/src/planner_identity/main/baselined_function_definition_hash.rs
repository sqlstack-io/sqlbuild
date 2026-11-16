//! The function's definition fingerprint, or its recorded one while the recorded definition still fingerprints the same.

use crate::planner_identity::_helpers::hashing::recorded_baseline;
use crate::planner_identity::main::function_definition_hash::function_definition_hash;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_analysis::sql_tokens::main::definition_fingerprint::definition_fingerprint;

/// The function's definition fingerprint, or its recorded one while the recorded definition still fingerprints the same.
pub fn baselined_function_definition_hash(
    py: Python<'_>,
    function_name: &str,
    (fingerprint_sql, language, dialect): (&str, &str, Option<&str>),
    recorded: &Bound<'_, PyAny>,
) -> PyResult<String> {
    let current: String =
        function_definition_hash(py, function_name, (fingerprint_sql, language, dialect))?;
    recorded_baseline(py, current, recorded, |sql| {
        definition_fingerprint(sql, language, dialect)
    })
}
