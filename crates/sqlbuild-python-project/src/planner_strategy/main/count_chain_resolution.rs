//! Count one batch resolution of SQL-test model chains.

use crate::planner_strategy::_helpers::fixtures::fixture_context::work_counts;
use pyo3::prelude::{PyResult, Python};

/// Count one batch resolution of SQL-test model chains.
pub fn count_chain_resolution(py: Python<'_>) -> PyResult<()> {
    work_counts(py)?.borrow_mut().chain_resolutions += 1;
    Ok(())
}
