//! Alter drifted maintained models, set their target lag, and mark kept readers of a replaced upstream.

use crate::planner_phases::_helpers::outputs::maintained;
use crate::planner_phases::models::ModelEntryResults;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// Alter drifted maintained models, set their target lag, and mark kept readers of a replaced upstream.
#[inline]
pub fn plan_maintained_refresh<'py>(
    runtime: &Bound<'py, PyAny>,
    execution_scope: &Bound<'py, PyAny>,
    entries: ModelEntryResults<'py>,
) -> PyResult<ModelEntryResults<'py>> {
    maintained::plan_maintained_refresh(runtime, execution_scope, entries)
}
