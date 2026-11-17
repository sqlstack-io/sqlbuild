//! An empty-fixture marker rendered as the relation's typed empty projection, or why it cannot be.

use crate::planner_strategy::_helpers::fixtures::fixture_sql::fixture_marker;
use crate::planner_strategy::models::FixtureRelation;
use pyo3::prelude::{Bound, PyAnyMethods, PyResult};
use sqlbuild_analysis::planner_fixtures::main::{
    empty_fixture_error::empty_fixture_error, render_empty::render_empty,
};
use sqlbuild_analysis::planner_fixtures::models::FixtureColumn;
use sqlbuild_python_base::native_host::models::Dialect;
use sqlbuild_python_base::native_host::types::Syntax;

/// An empty-fixture marker rendered as the relation's typed empty projection, or why it cannot be.
pub fn complete_empty_fixture_sql(
    sql: &str,
    relation: Option<&FixtureRelation>,
    dialect: &Bound<'_, Dialect>,
) -> PyResult<(String, Option<String>)> {
    if !fixture_marker(sql)? {
        return Ok((sql.into(), None));
    }
    let authoritative = relation.is_some_and(|relation| relation.authoritative);
    let metadata: &[FixtureColumn] = match relation {
        Some(relation) if authoritative => &relation.columns,
        _ => &[],
    };
    if let Some(error) = empty_fixture_error(authoritative, metadata) {
        return Ok((sql.into(), Some(error)));
    }
    Ok((
        render_empty(metadata, |name| {
            dialect
                .syntax("render_identifier")?
                .call1((name,))?
                .extract::<String>()
        })?,
        None,
    ))
}
