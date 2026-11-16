//! Build a model plan entry and its warnings from resolved plan decisions.

use crate::planner_strategy::_helpers::model_entries;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::types::PyTuple;
use sqlbuild_python_project::planner_strategy::models::ModelEntryDecisions;

/// Build a model plan entry and its warnings from resolved plan decisions.
#[inline]
pub fn assemble_model_plan_entry<'py>(
    request: &ModelEntryDecisions<'py>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyTuple>)> {
    model_entries::assemble_model_plan_entry(request)
}
