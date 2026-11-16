//! The plan action and reason for one model.

use crate::planner_strategy::_helpers::strategy;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyTuple;

/// The plan action and reason for one model.
#[inline]
pub fn resolve_model_plan_action<'py>(
    model: &Bound<'py, PyAny>,
    change_result: &Bound<'py, PyAny>,
    full_refresh: bool,
) -> PyResult<Bound<'py, PyTuple>> {
    strategy::resolve_model_plan_action(model, change_result, full_refresh)
}
