//! Stable function definition text used for change fingerprints.

use crate::planner_identity::main::compiled_function_fingerprint_sql::compiled_function_fingerprint_sql;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;

/// Stable function definition text used for change fingerprints.
#[pyfunction]
pub fn build_compiled_function_fingerprint_sql(function: &Bound<'_, PyAny>) -> PyResult<String> {
    compiled_function_fingerprint_sql(function)
}
