//! Build model plan entries from snapshot and per-model resolved actions.

use crate::planner_entries::_helpers::entries::model_entries;
use crate::planner_entries::models::PlanEntriesRequest;
use crate::planner_phases::models::ModelEntryResults;
use pyo3::prelude::PyResult;

/// Build model plan entries from snapshot and per-model resolved actions.
#[inline]
pub fn plan_entries<'py>(request: PlanEntriesRequest<'py>) -> PyResult<ModelEntryResults<'py>> {
    model_entries::plan_entries(request)
}
