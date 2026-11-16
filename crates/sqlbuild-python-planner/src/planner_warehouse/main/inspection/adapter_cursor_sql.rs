//! The query's MIN/MAX statement, each bound read as text the way the adapter renders it.

use crate::planner_warehouse::_helpers::inspection::cursors::CURSOR_TEXT_METHOD;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult};
use sqlbuild_analysis::planner_warehouse::main::cursor_query_sql::cursor_query_sql;
use sqlbuild_analysis::planner_warehouse::models::PhysicalCursorQuery;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_project::planner_phases::main::text::text;
use std::cell::RefCell;

/// The query's MIN/MAX statement, each bound read as text the way the adapter renders it.
pub fn adapter_cursor_sql(
    adapter: &Bound<'_, PyAny>,
    query: &PhysicalCursorQuery,
) -> PyResult<String> {
    let py = adapter.py();
    let render = native_field(adapter, CURSOR_TEXT_METHOD)?;
    let failure: RefCell<Option<PyErr>> = RefCell::new(None);
    let sql: String = cursor_query_sql(query, &|expression: &str| {
        call_with(
            &render,
            &[
                ("expression", text(py, expression)),
                ("cursor_type", text(py, &query.cursor_type)),
            ],
        )
        .and_then(|rendered| rendered.extract::<String>())
        .unwrap_or_else(|error| {
            failure.replace(Some(error));
            String::new()
        })
    });
    match failure.into_inner() {
        Some(error) => Err(error),
        None => Ok(sql),
    }
}
