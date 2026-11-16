//! The planning context of a project, over its models or the given name-to-model map.

use crate::planner_strategy::_helpers::fixtures::fixture_context;
use crate::planner_strategy::models::RelationFixturePlanningContext;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::types::PyDict;

/// The planning context of a project, over its models or the given name-to-model map.
#[inline]
pub fn planning_context(
    project: &Bound<'_, PyAny>,
    models_by_name: Option<Bound<'_, PyDict>>,
) -> PyResult<RelationFixturePlanningContext> {
    fixture_context::planning_context(project, models_by_name)
}
