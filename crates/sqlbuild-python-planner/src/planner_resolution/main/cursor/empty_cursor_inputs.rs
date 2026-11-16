//! Empty inputs that leave a planner-owned window empty, and whether others have rows.

use crate::planner_resolution::_helpers::cursor::cursor_ranges::INCREMENTAL;
use crate::planner_resolution::_helpers::cursor::cursor_ranges::ROLLING_WINDOW;
use crate::planner_resolution::_helpers::cursor::cursor_ranges::model_cursor_snapshot;
use crate::planner_resolution::_helpers::cursor::cursor_windows::cursor_inputs_without_rows;
use crate::planner_resolution::main::cursor::config_str::config_str;
use crate::planner_resolution::models::CursorWindowInputs;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::PyTuple;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Empty inputs that leave a planner-owned window empty, and whether others have rows.
pub fn empty_cursor_inputs<'py>(
    model: &Bound<'py, PyAny>,
    inputs: &CursorWindowInputs<'py>,
) -> PyResult<(Bound<'py, PyAny>, bool)> {
    let py = model.py();
    let nothing =
        || -> PyResult<(Bound<'py, PyAny>, bool)> { Ok((PyTuple::empty(py).into_any(), false)) };
    if inputs.full_refresh
        || inputs.runtime_owned
        || !config_str(model, "materialized")?.eq(INCREMENTAL)?
        || config_str(model, "cursor")?.is_none()
        || config_str(model, "microbatch_strategy")?.eq(ROLLING_WINDOW)?
        || (!inputs.overrides.0.is_none() && !inputs.overrides.1.is_none())
    {
        return nothing();
    }
    let cursor_snapshot = model_cursor_snapshot(model, &inputs.snapshot)?;
    if cursor_snapshot.is_none() {
        return nothing();
    }
    let decision = cursor_inputs_without_rows(&cursor_snapshot)?;
    Ok((
        native_field(&decision, "input_names")?,
        native_field(&decision, "waiting_on_empty_inputs")?.is_truthy()?,
    ))
}
