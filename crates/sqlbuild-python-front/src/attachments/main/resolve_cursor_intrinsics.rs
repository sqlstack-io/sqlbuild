//! Whether `sql` uses cursor intrinsics, and `sql` with both bounds rendered when given.

use crate::attachments::_helpers::cursor_intrinsics::CURSOR_END_CALL;
use crate::attachments::_helpers::cursor_intrinsics::CURSOR_START_CALL;
use crate::attachments::_helpers::cursor_intrinsics::SQL_CONTEXT;
use crate::attachments::_helpers::cursor_intrinsics::replaced;
use pyo3::prelude::PyResult;
use pyo3::{Python, pyfunction};

/// Whether `sql` uses cursor intrinsics, and `sql` with both bounds rendered when given.
#[pyfunction]
#[pyo3(signature = (*, sql, start_sql=None, end_sql=None))]
pub fn resolve_cursor_intrinsics(
    py: Python<'_>,
    sql: &str,
    start_sql: Option<&str>,
    end_sql: Option<&str>,
) -> PyResult<(String, bool)> {
    let (rendered, found) = replaced(
        py,
        sql,
        SQL_CONTEXT,
        (
            start_sql.unwrap_or(CURSOR_START_CALL),
            end_sql.unwrap_or(CURSOR_END_CALL),
        ),
    )?;
    if !found || start_sql.is_none() || end_sql.is_none() {
        return Ok((sql.to_owned(), found));
    }
    Ok((rendered, true))
}
