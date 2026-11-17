//! Adapter-bound rendering of fixture completion SQL.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_analysis::planner_fixtures::main::{
    empty_fixture_marker::empty_fixture_marker,
    render_completed::render_completed as native_render_completed,
};
use sqlbuild_analysis::planner_fixtures::models::{
    FixtureColumn, FixtureSqlInput, SuppliedProjection,
};
use sqlbuild_python_base::native_host::models::Dialect;

use sqlbuild_python_base::boundary::main::value_error::value_error;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

pub(crate) fn fixture_marker(sql: &str) -> PyResult<bool> {
    empty_fixture_marker(sql).map_err(value_error)
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
    dialect: &Bound<'_, Dialect>,
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
            dialect
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
