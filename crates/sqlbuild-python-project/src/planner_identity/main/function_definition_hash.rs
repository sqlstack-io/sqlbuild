//! The definition fingerprint of a function, or the planner input error naming the function.

use crate::planner_identity::_helpers::hashing;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// The definition fingerprint of a function, or the planner input error naming the function.
#[inline]
pub fn function_definition_hash(
    py: Python<'_>,
    function_name: &str,
    (fingerprint_sql, language, dialect): (&str, &str, Option<&str>),
) -> PyResult<String> {
    hashing::function_definition_hash(py, function_name, (fingerprint_sql, language, dialect))
}
