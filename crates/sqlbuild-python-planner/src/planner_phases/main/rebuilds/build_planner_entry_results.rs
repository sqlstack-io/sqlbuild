//! Model plan entries for the pruned execution scope, with the freshness and override inputs they need.

use crate::planner_phases::_helpers::rebuilds::execution_changes;
use crate::planner_phases::models::ChangeReconciliation;
use crate::planner_phases::models::ModelEntryResults;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// Model plan entries for the pruned execution scope, with the freshness and override inputs they need.
#[inline]
pub fn build_planner_entry_results<'py>(
    (runtime, warehouse): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    overrides: &Bound<'py, PyAny>,
    (execution_scope, reconciliation): (&Bound<'py, PyAny>, &ChangeReconciliation<'py>),
    source_freshness: &Bound<'py, PyAny>,
) -> PyResult<ModelEntryResults<'py>> {
    execution_changes::build_planner_entry_results(
        (runtime, warehouse),
        overrides,
        (execution_scope, reconciliation),
        source_freshness,
    )
}
