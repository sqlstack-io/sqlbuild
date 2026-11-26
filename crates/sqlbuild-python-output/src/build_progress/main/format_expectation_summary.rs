//! The few words a failed expectation's child row gives, for a Python step result.

use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_core::test_command::main::expectation_summary::expectation_summary;

use crate::build_progress::_helpers::progress_text::expectation_step;

/// `3 rows differ` or `2 rows` for a failed step; empty when it passed.
pub fn format_expectation_summary(step_result: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(expectation_summary(&expectation_step(step_result)?))
}
