//! Format a comparison result name as a user-facing expectation name.

use pyo3::pyfunction;
use sqlbuild_core::progress::main::format_expectation_name::format_expectation_name as format_name;

/// Format a comparison result name as a user-facing expectation name.
#[pyfunction]
pub fn format_expectation_name(model_name: &str) -> String {
    format_name(model_name)
}
