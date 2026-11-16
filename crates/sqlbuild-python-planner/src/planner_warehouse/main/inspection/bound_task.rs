//! Bound task.

use crate::planner_warehouse::_helpers::inspection::cursors;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// Bound task.
#[inline]
pub fn bound_task<'py>(
    py: Python<'py>,
    sql: String,
    (connection, execute): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    cursors::bound_task(py, sql, (connection, execute))
}
