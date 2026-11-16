//! Build the execution plan for one selection against live warehouse state.

use crate::planner_entries::_helpers::plans::execution_plan;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyDict;

/// Build the execution plan for one selection against live warehouse state.
#[inline]
pub fn build_execution_plan<'py>(
    py: Python<'py>,
    kwargs: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    execution_plan::build_execution_plan(py, kwargs)
}
