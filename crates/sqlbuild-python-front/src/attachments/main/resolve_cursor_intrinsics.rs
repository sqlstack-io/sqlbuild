//! Whether `sql` uses cursor intrinsics, and `sql` with both bounds rendered when given.

use crate::attachments::_helpers::cursor_intrinsics;
use pyo3::Python;
use pyo3::prelude::PyResult;

/// Whether `sql` uses cursor intrinsics, and `sql` with both bounds rendered when given.
#[inline]
pub fn resolve_cursor_intrinsics(
    py: Python<'_>,
    sql: &str,
    start_sql: Option<&str>,
    end_sql: Option<&str>,
) -> PyResult<(String, bool)> {
    cursor_intrinsics::resolve_cursor_intrinsics(py, sql, start_sql, end_sql)
}
