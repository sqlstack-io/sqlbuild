//! Each selected model's change with the backfill its own `replay_on_change` decided.

use crate::planner_changes::_helpers::actions;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Each selected model's change with the backfill its own `replay_on_change` decided.
#[inline]
pub fn resolve_model_actions<'py>(
    scope: &Bound<'py, PyAny>,
    changes: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    actions::resolve_model_actions(scope, changes)
}
