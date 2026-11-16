//! Compute cursor bounds for inclusion on the plan entry.

use crate::planner_resolution::_helpers::cursor::cursor_ranges::BoundsRequest;
use crate::planner_resolution::_helpers::cursor::cursor_ranges::FULL_BACKFILL;
use crate::planner_resolution::_helpers::cursor::cursor_ranges::INCREMENTAL;
use crate::planner_resolution::_helpers::cursor::cursor_ranges::MICROBATCH;
use crate::planner_resolution::_helpers::cursor::cursor_ranges::bounded_override;
use crate::planner_resolution::_helpers::cursor::cursor_ranges::clamp_cursor_end;
use crate::planner_resolution::_helpers::cursor::cursor_ranges::model_cursor_snapshot;
use crate::planner_resolution::_helpers::cursor::cursor_ranges::snapshot_bounds;
use crate::planner_resolution::_helpers::cursor::cursor_ranges::without_destination;
use crate::planner_resolution::main::cursor::config_str::config_str;
use crate::planner_resolution::models::CursorWindowInputs;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Compute cursor bounds for inclusion on the plan entry.
pub fn plan_cursor_bounds<'py>(
    model: &Bound<'py, PyAny>,
    inputs: &CursorWindowInputs<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = model.py();
    if !config_str(model, "materialized")?.eq(INCREMENTAL)?
        || config_str(model, "cursor")?.is_none()
    {
        return Ok(py.None().into_bound(py));
    }
    let bounded = bounded_override(model, inputs)?;
    if !bounded.is_none() {
        return clamp_cursor_end(bounded, model);
    }
    if inputs.runtime_owned || inputs.full_refresh {
        return Ok(py.None().into_bound(py));
    }
    let mut cursor_snapshot = model_cursor_snapshot(model, &inputs.snapshot)?;
    if cursor_snapshot.is_none() {
        return Ok(cursor_snapshot);
    }
    if native_field(&inputs.backfill, "action")?.eq(FULL_BACKFILL)? {
        cursor_snapshot = without_destination(cursor_snapshot)?;
    }
    snapshot_bounds(
        model,
        inputs,
        BoundsRequest {
            cursor_snapshot,
            typing: (
                config_str(model, "cursor_type")?,
                config_str(model, "cursor_grain")?,
            ),
            lookback: config_str(model, "lookback")?,
            is_microbatch: config_str(model, "incremental_mode")?.eq(MICROBATCH)?,
        },
    )
}
