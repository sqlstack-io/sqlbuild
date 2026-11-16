//! A fully bounded inclusive operator override as an executable interval.

use crate::plan_output::_helpers::display::cursor_typing;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::cursor::main::scalars::to_exclusive::to_exclusive;
use sqlbuild_python_front::cursor::main::scalars::try_parse::try_parse;

/// A fully bounded inclusive operator override as an executable interval.
pub fn resolve_bounded_cursor_override<'py>(
    start_cursor_override: &Bound<'py, PyAny>,
    end_cursor_override: &Bound<'py, PyAny>,
    cursor_type: &Bound<'py, PyAny>,
    cursor_grain: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let py: Python<'py> = start_cursor_override.py();
    if start_cursor_override.is_none() || end_cursor_override.is_none() {
        return Ok(None);
    }
    let (effective, grain) = cursor_typing(cursor_type, cursor_grain)?;
    let (Some(start), Some(end)) = (
        try_parse(start_cursor_override, &effective)?,
        try_parse(end_cursor_override, &effective)?,
    ) else {
        return Ok(None);
    };
    new_record_with(
        py,
        RecordId::CursorBounds,
        &[("start", start), ("end", to_exclusive(&end, &grain)?)],
    )
    .map(Some)
}
