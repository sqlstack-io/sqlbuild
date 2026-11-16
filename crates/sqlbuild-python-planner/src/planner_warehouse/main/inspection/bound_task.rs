//! Bound task.

use crate::planner_warehouse::_helpers::inspection::cursors::read_cursor_query;
use pyo3::prelude::{Bound, Py, PyAny, PyResult, Python};
use pyo3::types::{PyCFunction, PyDict, PyTuple};

/// Bound task.
pub fn bound_task<'py>(
    py: Python<'py>,
    sql: String,
    (connection, execute): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    let connection: Py<PyAny> = connection.clone().unbind();
    let execute: Py<PyAny> = execute.clone().unbind();
    Ok(PyCFunction::new_closure(
        py,
        None,
        None,
        move |args: &Bound<'_, PyTuple>,
              _kwargs: Option<&Bound<'_, PyDict>>|
              -> PyResult<Py<PyAny>> {
            let py = args.py();
            Ok(read_cursor_query(py, &sql, (connection.bind(py), execute.bind(py)))?.unbind())
        },
    )?
    .into_any())
}
