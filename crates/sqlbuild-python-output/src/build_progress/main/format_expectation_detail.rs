//! Format failing row count detail for an expectation row.

use crate::build_progress::_helpers::progress_text::expectation_step;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;
use sqlbuild_core::progress::main::format_expectation_detail::format_expectation_detail as format_detail;

/// Format failing row count detail for an expectation row.
#[pyfunction]
pub fn format_expectation_detail(step_result: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(format_detail(&expectation_step(step_result)?))
}
