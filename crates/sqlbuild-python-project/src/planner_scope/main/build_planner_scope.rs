//! Build planner scope.

use crate::planner_scope::_helpers::scope;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::PyDict;

/// Build planner scope.
#[inline]
pub fn build_planner_scope<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    scope::build_planner_scope(py, options)
}
