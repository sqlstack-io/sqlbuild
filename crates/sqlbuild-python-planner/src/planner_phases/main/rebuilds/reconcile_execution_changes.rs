//! The execution scope's changes with honest write hashes, and the actions merged from them.

use crate::planner_phases::_helpers::rebuilds::execution_changes;
use crate::planner_phases::models::ChangeReconciliation;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The execution scope's changes with honest write hashes, and the actions merged from them.
#[inline]
pub fn reconcile_execution_changes<'py>(
    (execution_scope, snapshot): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    identities: &Bound<'py, PyAny>,
    resolved_actions: &Bound<'py, PyAny>,
    changes: &Bound<'py, PyAny>,
) -> PyResult<ChangeReconciliation<'py>> {
    execution_changes::reconcile_execution_changes(
        (execution_scope, snapshot),
        identities,
        resolved_actions,
        changes,
    )
}
