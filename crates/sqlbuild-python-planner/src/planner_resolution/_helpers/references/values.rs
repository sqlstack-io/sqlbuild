//! Boundary calls shared by reference and source resolution.

use crate::planner_resolution::models::CursorFiltering;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyString;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::native_host::models::Dialect;
use sqlbuild_python_base::native_host::types::Syntax;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_front::cursor::main::scalars::sentinel_to_token::sentinel_to_token;
use sqlbuild_sqltext::resolution::main::cursor_subquery::cursor_subquery;
use sqlbuild_sqltext::resolution::models::CursorFilter;

pub(crate) fn string<'py>(py: Python<'py>, text: &str) -> Bound<'py, PyAny> {
    PyString::new(py, text).into_any()
}

pub(crate) fn method<'py>(
    object: &Bound<'py, PyAny>,
    name: &str,
    values: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    call_with(&native_field(object, name)?, values)
}

pub(crate) fn cursor_wrap(
    dialect: &Bound<'_, Dialect>,
    filtering: &CursorFiltering<'_>,
    relation: String,
    (name, inject_alias): (&str, bool),
) -> PyResult<String> {
    let bounds = &filtering.bounds;
    if bounds.is_none() {
        return Ok(relation);
    }
    let column: Option<String> = mapping_get(&filtering.filter_inputs, name)?.extract()?;
    let Some(column) = column else {
        return Ok(relation);
    };
    let py = dialect.py();
    let cursor_type = &filtering.cursor_type;
    let literal = |bound: &str| -> PyResult<String> {
        let token = sentinel_to_token(&native_field(bounds, bound)?)?;
        method(
            dialect,
            "render_cursor_bound_literal",
            &[
                ("value", string(py, &token)),
                ("cursor_type", cursor_type.clone()),
            ],
        )?
        .extract()
    };
    let start = literal("start")?;
    let end = literal("end")?;
    let operand = cursor_operand(dialect, &column, cursor_type)?;
    let alias = inject_alias
        && dialect
            .syntax("requires_derived_table_aliases")?
            .call0()?
            .is_truthy()?;
    Ok(cursor_subquery(
        &relation,
        &CursorFilter {
            column: &operand,
            start: &start,
            end: &end,
            lower_inclusive: filtering.lower_bound_inclusive,
            typed: !cursor_type.is_none(),
        },
        alias,
    ))
}

/// The adapter's expression for a cursor column compared with bounds, or the column itself.
fn cursor_operand(
    dialect: &Bound<'_, Dialect>,
    column: &str,
    cursor_type: &Bound<'_, PyAny>,
) -> PyResult<String> {
    let py = dialect.py();
    if !dialect.hasattr("render_cursor_operand")? {
        return Ok(column.to_owned());
    }
    method(
        dialect,
        "render_cursor_operand",
        &[
            ("expression", string(py, column)),
            ("cursor_type", cursor_type.clone()),
        ],
    )?
    .extract()
}

/// The cursor filtering of a keyword-argument resolution request.
pub(crate) fn filtering_of<'py>(
    arguments: &KeywordArguments<'py>,
) -> PyResult<CursorFiltering<'py>> {
    Ok(CursorFiltering {
        bounds: arguments.value("cursor_bounds")?,
        filter_inputs: arguments.value("cursor_filter_inputs")?,
        cursor_type: arguments.value("cursor_type")?,
        lower_bound_inclusive: arguments.value("lower_bound_inclusive")?.is_truthy()?,
    })
}
