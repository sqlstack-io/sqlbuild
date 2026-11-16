//! Reject authored cursor input columns a contract or the source warehouse metadata does not expose.

use crate::planner_strategy::_helpers::cursor_inputs;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_python_project::planner_strategy::models::CursorInputLookups;

/// Reject authored cursor input columns a contract or the source warehouse metadata does not expose.
#[inline]
pub fn reject_unexposed_cursor_input_columns<'py>(
    model: &Bound<'py, PyAny>,
    cursor_column: &Bound<'py, PyAny>,
    lookups: &CursorInputLookups<'py>,
) -> PyResult<()> {
    cursor_inputs::reject_unexposed_cursor_input_columns(model, cursor_column, lookups)
}
