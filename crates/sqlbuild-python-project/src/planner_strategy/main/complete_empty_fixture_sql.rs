//! An empty-fixture marker rendered as the relation's typed empty projection, or why it cannot be.

use crate::planner_strategy::_helpers::fixtures::fixture_sql::fixture_marker;
use crate::planner_strategy::models::FixtureRelation;
<<<<<<< HEAD
use pyo3::prelude::{Bound, PyAnyMethods, PyResult};
=======
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
>>>>>>> origin/main
use sqlbuild_analysis::planner_fixtures::main::{
    empty_fixture_error::empty_fixture_error, render_empty::render_empty,
};
use sqlbuild_analysis::planner_fixtures::models::FixtureColumn;
<<<<<<< HEAD
use sqlbuild_python_base::native_host::models::Dialect;
use sqlbuild_python_base::native_host::types::Syntax;
=======
>>>>>>> origin/main

/// An empty-fixture marker rendered as the relation's typed empty projection, or why it cannot be.
pub fn complete_empty_fixture_sql(
    sql: &str,
    relation: Option<&FixtureRelation>,
<<<<<<< HEAD
    dialect: &Bound<'_, Dialect>,
=======
    adapter: &Bound<'_, PyAny>,
>>>>>>> origin/main
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
<<<<<<< HEAD
            dialect
                .syntax("render_identifier")?
                .call1((name,))?
=======
            adapter
                .call_method1("render_identifier", (name,))?
>>>>>>> origin/main
                .extract::<String>()
        })?,
        None,
    ))
}
