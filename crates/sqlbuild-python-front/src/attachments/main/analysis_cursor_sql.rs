//! `sql` with intrinsics replaced by stable typed literals, only for static SQL analysis.

use crate::attachments::_helpers::cursor_intrinsics;
use pyo3::Python;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// `sql` with intrinsics replaced by stable typed literals, only for static SQL analysis.
#[inline]
pub fn analysis_cursor_sql(
    py: Python<'_>,
    sql: &str,
    cursor_type: &Bound<'_, PyAny>,
) -> PyResult<String> {
    cursor_intrinsics::analysis_cursor_sql(py, sql, cursor_type)
}
