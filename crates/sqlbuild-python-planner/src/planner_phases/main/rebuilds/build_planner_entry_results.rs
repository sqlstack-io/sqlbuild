//! Model plan entries for the pruned execution scope, with the freshness and override inputs they need.

use crate::planner_phases::_helpers::rebuilds::execution_changes;
<<<<<<< HEAD
use crate::planner_phases::models::ChangeReconciliation;
use crate::planner_phases::models::ModelEntryResults;
=======
use crate::planner_phases::models::{ChangeReconciliation, ModelEntryResults};
>>>>>>> origin/main
use pyo3::prelude::{Bound, PyAny, PyResult};

/// Model plan entries for the pruned execution scope, with the freshness and override inputs they need.
#[inline]
pub fn build_planner_entry_results<'py>(
    (runtime, warehouse): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
<<<<<<< HEAD
    overrides: &Bound<'py, PyAny>,
=======
    (overrides, deferral): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
>>>>>>> origin/main
    (execution_scope, reconciliation): (&Bound<'py, PyAny>, &ChangeReconciliation<'py>),
    source_freshness: &Bound<'py, PyAny>,
) -> PyResult<ModelEntryResults<'py>> {
    execution_changes::build_planner_entry_results(
        (runtime, warehouse),
<<<<<<< HEAD
        overrides,
=======
        (overrides, deferral),
>>>>>>> origin/main
        (execution_scope, reconciliation),
        source_freshness,
    )
}
