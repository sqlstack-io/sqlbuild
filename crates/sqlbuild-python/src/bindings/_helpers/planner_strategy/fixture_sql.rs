//! Adapter-bound rendering of fixture completion SQL.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_analysis::planner_fixtures::main::{
    empty_fixture_error::empty_fixture_error, empty_fixture_marker::empty_fixture_marker,
    render_completed::render_completed as native_render_completed, render_empty::render_empty,
};
use sqlbuild_analysis::planner_fixtures::models::{
    FixtureColumn, FixtureSqlInput, SuppliedProjection,
};

use crate::bindings::_helpers::planner_strategy::models::FixtureRelation;
use sqlbuild_python_base::boundary::main::value_error::value_error;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

pub(crate) fn fixture_marker(sql: &str) -> PyResult<bool> {
    empty_fixture_marker(sql).map_err(value_error)
}

/// An empty-fixture marker rendered as the relation's typed empty projection, or why it cannot be.
pub(crate) fn complete_empty_fixture_sql(
    sql: &str,
    relation: Option<&FixtureRelation>,
    adapter: &Bound<'_, PyAny>,
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
            adapter
                .call_method1("render_identifier", (name,))?
                .extract::<String>()
        })?,
        None,
    ))
}

pub(crate) struct CompletionSql<'py, 'a> {
    pub(crate) sql: &'a str,
    pub(crate) inferred: Bound<'py, PyAny>,
    pub(crate) typed_nulls: Bound<'py, PyDict>,
    pub(crate) quoted_names: Bound<'py, PyAny>,
    pub(crate) completed: Vec<FixtureColumn>,
}

pub(crate) fn render_completed(
    input: CompletionSql<'_, '_>,
    adapter: &Bound<'_, PyAny>,
) -> PyResult<String> {
    let mut supplied: Vec<SuppliedProjection> = Vec::new();
    if !input.typed_nulls.is_empty() {
        for column in input.inferred.try_iter()? {
            let column = column?;
            let name = native_field(&column, "name")?;
            let folded = name.call_method0("casefold")?;
            supplied.push(SuppliedProjection {
                name: name.extract()?,
                quoted: input.quoted_names.contains(&folded)?,
                expected_type: input
                    .typed_nulls
                    .get_item(folded)?
                    .map(|value| value.extract())
                    .transpose()?,
            });
        }
    }
    native_render_completed(
        FixtureSqlInput {
            sql: input.sql,
            supplied: &supplied,
            typed_nulls: !input.typed_nulls.is_empty(),
            completed: &input.completed,
        },
        |name, exact| {
            adapter
                .call_method1(
                    if exact {
                        "render_exact_identifier"
                    } else {
                        "render_identifier"
                    },
                    (name,),
                )?
                .extract::<String>()
        },
    )
}

/// Fixture SQL with its untyped null columns cast to the expected types.
pub(crate) fn complete_typed_null_columns(
    sql: &str,
    inference: &Bound<'_, PyAny>,
    expected_types: &[(String, String)],
    adapter: &Bound<'_, PyAny>,
) -> PyResult<String> {
    let py = inference.py();
    let typed_nulls = PyDict::new(py);
    for name in native_field(inference, "null_literal_names")?.try_iter()? {
        let name = name?;
        let folded: String = name.extract()?;
        if let Some((_, expected)) = expected_types.iter().find(|(column, _)| *column == folded) {
            typed_nulls.set_item(name, expected)?;
        }
    }
    if typed_nulls.is_empty() {
        return Ok(sql.into());
    }
    render_completed(
        CompletionSql {
            sql,
            inferred: native_field(inference, "columns")?,
            typed_nulls,
            quoted_names: native_field(inference, "quoted_names")?,
            completed: Vec::new(),
        },
        adapter,
    )
}
