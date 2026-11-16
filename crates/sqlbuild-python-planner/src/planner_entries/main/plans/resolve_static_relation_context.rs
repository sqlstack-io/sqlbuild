//! Resolve canonical locations and source routing without warehouse inventory.

use crate::planner_entries::_helpers::plans::command_plans;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyDict;

/// Resolve canonical locations and source routing without warehouse inventory.
#[inline]
pub fn resolve_static_relation_context<'py>(
    py: Python<'py>,
    kwargs: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    command_plans::resolve_static_relation_context(py, kwargs)
}
