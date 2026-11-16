//! `sql` with intrinsics replaced by stable typed literals, only for static SQL analysis.

use crate::attachments::_helpers::cursor_intrinsics::MODEL_SQL_CONTEXT;
use crate::attachments::_helpers::cursor_intrinsics::analysis_bound;
use crate::attachments::_helpers::cursor_intrinsics::replaced;
use crate::attachments::_helpers::cursor_intrinsics::text_value;
use pyo3::Python;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// `sql` with intrinsics replaced by stable typed literals, only for static SQL analysis.
pub fn analysis_cursor_sql(
    py: Python<'_>,
    sql: &str,
    cursor_type: &Bound<'_, PyAny>,
) -> PyResult<String> {
    let literal: &str = analysis_bound(text_value(cursor_type).as_deref());
    Ok(replaced(py, sql, MODEL_SQL_CONTEXT, (literal, literal))?.0)
}
