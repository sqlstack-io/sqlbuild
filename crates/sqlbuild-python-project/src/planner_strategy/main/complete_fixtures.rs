//! Complete the fixture groups of one chain against the planning context.

use crate::planner_strategy::_helpers::fixtures::fixture_completion;
use crate::planner_strategy::models::FixtureCompletion;
use crate::planner_strategy::models::RelationFixturePlanningContext;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Complete the fixture groups of one chain against the planning context.
#[inline]
pub fn complete_fixtures<'py, 'a>(
    context: &'a RelationFixturePlanningContext,
    adapter: &Bound<'py, PyAny>,
    (ordered_model_names, fixture_groups): (&[String], &Bound<'py, PyAny>),
) -> PyResult<FixtureCompletion<'py, 'a>> {
    fixture_completion::complete_fixtures(context, adapter, (ordered_model_names, fixture_groups))
}
