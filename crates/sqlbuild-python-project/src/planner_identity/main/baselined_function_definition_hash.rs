//! The function's definition fingerprint, or its recorded one while the recorded definition still fingerprints the same.

use crate::planner_identity::_helpers::hashing;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// The function's definition fingerprint, or its recorded one while the recorded definition still fingerprints the same.
#[inline]
pub fn baselined_function_definition_hash(
    py: Python<'_>,
    function_name: &str,
    (fingerprint_sql, language, dialect): (&str, &str, Option<&str>),
    recorded: &Bound<'_, PyAny>,
) -> PyResult<String> {
    hashing::baselined_function_definition_hash(
        py,
        function_name,
        (fingerprint_sql, language, dialect),
        recorded,
    )
}
