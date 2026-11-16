//! The `CursorType` a supported cross-adapter SQL type holds, or `None`.

use crate::planner_changes::main::planner_enum::planner_enum;
use crate::planner_strategy::_helpers::cursor_types::log_parse_failure;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_analysis::planner_strategy::main::classify_cursor_sql_type::classify_cursor_sql_type as classify;
use sqlbuild_analysis::planner_strategy::models::CursorType;
use sqlbuild_python_base::records::types::RecordId;

/// The `CursorType` a supported cross-adapter SQL type holds, or `None`.
pub fn classify_cursor_sql_type<'py>(
    py: Python<'py>,
    sql_type: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let detected: Option<CursorType> = match classify(sql_type) {
        Ok(detected) => detected,
        Err(failure) => {
            log_parse_failure(py, &failure)?;
            None
        }
    };
    detected
        .map(|cursor_type| {
            let value: &str = match cursor_type {
                CursorType::Timestamp => "timestamp",
                CursorType::Integer => "integer",
            };
            planner_enum(py, RecordId::CursorType, value)
        })
        .transpose()
}
