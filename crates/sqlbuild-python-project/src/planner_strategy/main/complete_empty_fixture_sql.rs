//! An empty-fixture marker rendered as the relation's typed empty projection, or why it cannot be.

use crate::planner_strategy::_helpers::fixture_sql;
use crate::planner_strategy::models::FixtureRelation;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// An empty-fixture marker rendered as the relation's typed empty projection, or why it cannot be.
#[inline]
pub fn complete_empty_fixture_sql(
    sql: &str,
    relation: Option<&FixtureRelation>,
    adapter: &Bound<'_, PyAny>,
) -> PyResult<(String, Option<String>)> {
    fixture_sql::complete_empty_fixture_sql(sql, relation, adapter)
}
