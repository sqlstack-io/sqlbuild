//! A cursor bound for display, collapsing a whole-day midnight timestamp to a date.

use crate::plan_output::_helpers::display::parsed_bound;
use crate::plan_output::_helpers::values::optional_text;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use sqlbuild_core::plan_display::constants::INTEGER_CURSOR;
use sqlbuild_core::plan_display::main::cursor_bound_display::cursor_bound_display as bound_display;
use sqlbuild_core::plan_display::models::ParsedBound;

/// A cursor bound for display, collapsing a whole-day midnight timestamp to a date.
#[pyfunction]
#[pyo3(signature = (*, value, cursor_type, cursor_grain))]
pub fn cursor_bound_display(
    value: &Bound<'_, PyAny>,
    cursor_type: &Bound<'_, PyAny>,
    cursor_grain: &Bound<'_, PyAny>,
) -> PyResult<String> {
    let cursor_type: Option<String> = optional_text(cursor_type)?;
    let raw: String = value.extract()?;
    let parsed: Option<ParsedBound> = if cursor_type.as_deref() == Some(INTEGER_CURSOR) {
        None
    } else {
        parsed_bound(value)?
    };
    Ok(bound_display(
        &raw,
        (
            cursor_type.as_deref(),
            optional_text(cursor_grain)?.as_deref(),
        ),
        parsed.as_ref(),
    ))
}
