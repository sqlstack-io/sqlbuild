//! The query's MIN/MAX statement, each bound read as text the way the adapter renders it.

use crate::planner_warehouse::_helpers::inspection::cursors;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_analysis::planner_warehouse::models::PhysicalCursorQuery;

/// The query's MIN/MAX statement, each bound read as text the way the adapter renders it.
#[inline]
pub fn adapter_cursor_sql(
    adapter: &Bound<'_, PyAny>,
    query: &PhysicalCursorQuery,
) -> PyResult<String> {
    cursors::adapter_cursor_sql(adapter, query)
}
