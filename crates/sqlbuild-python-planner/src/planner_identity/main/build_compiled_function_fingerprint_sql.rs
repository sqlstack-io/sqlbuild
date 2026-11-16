//! Stable function definition text used for change fingerprints.

use crate::planner_identity::_helpers::metadata;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Stable function definition text used for change fingerprints.
#[inline]
pub fn build_compiled_function_fingerprint_sql(function: &Bound<'_, PyAny>) -> PyResult<String> {
    metadata::build_compiled_function_fingerprint_sql(function)
}
