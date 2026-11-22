//! Cursor markers replaced with concrete bounds when a statement runs.

use crate::attachments::_helpers::cursor_intrinsics::markers_replaced;
use pyo3::pyfunction;

/// `sql` with the start and end cursor markers replaced by `start_sql` and `end_sql`.
#[pyfunction]
#[pyo3(signature = (*, sql, start_sql, end_sql))]
pub fn replace_cursor_markers(sql: &str, start_sql: &str, end_sql: &str) -> String {
    markers_replaced(sql, (start_sql, end_sql))
}
