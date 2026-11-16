//! The `CursorType` a supported cross-adapter SQL type holds, or `None`.

use crate::planner_strategy::_helpers::cursor_types;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// The `CursorType` a supported cross-adapter SQL type holds, or `None`.
#[inline]
pub fn classify_cursor_sql_type<'py>(
    py: Python<'py>,
    sql_type: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    cursor_types::classify_cursor_sql_type(py, sql_type)
}
