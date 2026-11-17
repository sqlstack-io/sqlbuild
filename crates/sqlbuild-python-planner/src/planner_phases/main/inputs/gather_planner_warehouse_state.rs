//! Gather planner warehouse state.

use crate::planner_phases::_helpers::inputs::warehouse_state;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// Gather planner warehouse state.
#[inline]
pub fn gather_planner_warehouse_state<'py>(
    runtime: &Bound<'py, PyAny>,
    scopes: &Bound<'py, PyAny>,
    overrides: &Bound<'py, PyAny>,
    deferral: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    warehouse_state::gather_planner_warehouse_state(runtime, scopes, overrides, deferral)
}
