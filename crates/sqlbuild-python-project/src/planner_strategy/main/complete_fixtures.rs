//! Complete the fixture groups of one chain against the planning context.

use crate::planner_strategy::_helpers::fixtures::fixture_completion::complete_prepared;
use crate::planner_strategy::_helpers::fixtures::fixture_preparation::prepare;
use crate::planner_strategy::_helpers::fixtures::fixture_reads::required_fixture_columns;
use crate::planner_strategy::_helpers::fixtures::fixture_selection::fixture_keys_requiring_analysis;
use crate::planner_strategy::_helpers::fixtures::fixture_types::expected_fixture_types;
use crate::planner_strategy::models::FixtureCompletion;
use crate::planner_strategy::models::RelationFixturePlanningContext;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// Complete the fixture groups of one chain against the planning context.
pub fn complete_fixtures<'py, 'a>(
    context: &'a RelationFixturePlanningContext,
    adapter: &Bound<'py, PyAny>,
    (ordered_model_names, fixture_groups): (&[String], &Bound<'py, PyAny>),
) -> PyResult<FixtureCompletion<'py, 'a>> {
    let py = adapter.py();
    let prepared = prepare(fixture_groups, context, adapter)?;
    let fixture_keys = fixture_keys_requiring_analysis(&prepared, context)?;
    let required = required_fixture_columns(context, ordered_model_names, &fixture_keys, adapter)?;
    let expected = expected_fixture_types(context, py, ordered_model_names)?;
    complete_prepared(prepared, (context, &required), expected, adapter)
}
