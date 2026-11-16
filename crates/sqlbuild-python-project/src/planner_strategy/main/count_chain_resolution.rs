//! Count one batch resolution of SQL-test model chains.

use crate::planner_strategy::_helpers::fixtures::fixture_context;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// Count one batch resolution of SQL-test model chains.
#[inline]
pub fn count_chain_resolution(py: Python<'_>) -> PyResult<()> {
    fixture_context::count_chain_resolution(py)
}
