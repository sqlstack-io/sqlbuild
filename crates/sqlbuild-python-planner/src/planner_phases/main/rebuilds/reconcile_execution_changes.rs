//! The execution scope's changes with honest write hashes, and the actions merged from them.

use crate::planner_identity::main::merge_recomputed_model_changes::merge_recomputed_model_changes;
use crate::planner_identity::main::with_honest_model_write_hashes::with_honest_model_write_hashes;
use crate::planner_phases::_helpers::inputs::contexts::class_field;
use crate::planner_phases::models::ChangeReconciliation;
use crate::planner_phases::models::PlannerIdentityContext;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_python_base::boundary::main::keywords::keywords;

/// The execution scope's changes with honest write hashes, and the actions merged from them.
pub fn reconcile_execution_changes<'py>(
    (execution_scope, snapshot): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    identities: &Bound<'py, PyAny>,
    resolved_actions: &Bound<'py, PyAny>,
    changes: &Bound<'py, PyAny>,
) -> PyResult<ChangeReconciliation<'py>> {
    let py = execution_scope.py();
    let honest = with_honest_model_write_hashes(
        py,
        Some(keywords(
            py,
            &[
                ("scope", execution_scope.clone()),
                ("snapshot", snapshot.clone()),
                ("changes", changes.clone()),
                (
                    "version_identities",
                    class_field(identities, |i: &PlannerIdentityContext| {
                        &i.version_identities
                    })?,
                ),
            ],
        )?),
    )?;
    let resolved = merge_recomputed_model_changes(resolved_actions, &honest)?;
    Ok(ChangeReconciliation {
        changes: honest,
        resolved_actions: resolved,
    })
}
