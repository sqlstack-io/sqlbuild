//! Resolved actions with recomputed model changes patched in where available.

use crate::planner_identity::_helpers::identities;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Resolved actions with recomputed model changes patched in where available.
#[inline]
pub fn merge_recomputed_model_changes<'py>(
    resolved_actions: &Bound<'py, PyAny>,
    changes: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    identities::merge_recomputed_model_changes(resolved_actions, changes)
}
