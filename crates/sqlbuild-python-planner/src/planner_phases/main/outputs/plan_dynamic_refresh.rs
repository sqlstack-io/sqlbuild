//! Alter drifted dynamic models, set their target lag, and mark kept readers of a replaced upstream.

use crate::planner_phases::_helpers::outputs::dynamic;
use crate::planner_phases::models::ModelEntryResults;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// Alter drifted dynamic models, set their target lag, and mark kept readers of a replaced upstream.
#[inline]
pub fn plan_dynamic_refresh<'py>(
    runtime: &Bound<'py, PyAny>,
    execution_scope: &Bound<'py, PyAny>,
    entries: ModelEntryResults<'py>,
) -> PyResult<ModelEntryResults<'py>> {
    dynamic::plan_dynamic_refresh(runtime, execution_scope, entries)
}
