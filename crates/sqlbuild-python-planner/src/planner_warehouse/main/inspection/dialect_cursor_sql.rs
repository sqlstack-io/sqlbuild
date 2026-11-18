//! A physical cursor query rendered in a plan's SQL dialect.

use crate::planner_warehouse::_helpers::inspection::cursors;
use pyo3::prelude::{Bound, PyResult};
use sqlbuild_analysis::planner_warehouse::models::PhysicalCursorQuery;
use sqlbuild_python_base::native_host::models::Dialect;

/// A physical cursor query rendered in a plan's SQL dialect.
#[inline]
pub fn dialect_cursor_sql(
    dialect: &Bound<'_, Dialect>,
    query: &PhysicalCursorQuery,
) -> PyResult<String> {
    cursors::dialect_cursor_sql(dialect, query)
}
