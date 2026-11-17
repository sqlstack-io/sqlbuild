//! Warehouse reads answered live through the adapter.

use crate::live_reads::constants::QUERY_RECORD;
use crate::live_reads::constants::RECORD_QUERY;
use crate::live_reads::constants::RUN_RECORDED_QUERY;
use crate::planner_warehouse::constants::RUN_BOUNDED;
use crate::planner_warehouse::models::EligibleCursorQuery;
use pyo3::IntoPyObject;
use pyo3::exceptions::PyException;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyCFunction, PyDict, PyFloat, PyString, PyTuple};
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_project::planner_phases::main::planner_error::planner_error;
use std::time::Instant;

pub(crate) fn text<'py>(py: Python<'py>, value: &str) -> Bound<'py, PyAny> {
    PyString::new(py, value).into_any()
}

/// Run one cursor statement; a failure leaves only that relation unavailable.
pub(crate) fn read_cursor_query<'py>(
    py: Python<'py>,
    sql: &str,
    (connection, execute): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    let started = Instant::now();
    let outcome = call_with(
        execute,
        &[("connection", connection.clone()), ("sql", text(py, sql))],
    )
    .and_then(|result| result.call_method0("fetchall"));
    let elapsed = started.elapsed().as_secs_f64();
    let (row, error, count) = match outcome {
        Ok(rows) => {
            let count = rows.len()?;
            let row = if count > 0 {
                rows.get_item(0)?
            } else {
                py.None().into_bound(py)
            };
            (
                row,
                py.None().into_bound(py),
                count.into_pyobject(py)?.into_any(),
            )
        }
        Err(error) if error.is_instance_of::<PyException>(py) => {
            let value = error.value(py).clone().into_any();
            (py.None().into_bound(py), value, py.None().into_bound(py))
        }
        Err(error) => return Err(error),
    };
    let error_text = if error.is_none() {
        py.None().into_bound(py)
    } else {
        error.str()?.into_any()
    };
    let record = call_with(
        &host_entry(py, QUERY_RECORD)?,
        &[
            ("sql", text(py, sql)),
            ("elapsed_seconds", PyFloat::new(py, elapsed).into_any()),
            ("row_count", count),
            ("error", error_text),
        ],
    )?;
    call_with(&host_entry(py, RECORD_QUERY)?, &[("record", record)])?;
    Ok(PyTuple::new(py, [row, PyFloat::new(py, elapsed).into_any(), error])?.into_any())
}

/// A bounded-inspection task running one cursor statement.
pub(crate) fn bound_task<'py>(
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

/// A bounded-inspection task reading one model's highest eligible target cursor.
pub(crate) fn eligible_task<'py>(
    py: Python<'py>,
    query: EligibleCursorQuery,
    (connection, execute): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    let connection: Py<PyAny> = connection.clone().unbind();
    let execute: Py<PyAny> = execute.clone().unbind();
    Ok(PyCFunction::new_closure(
        py,
        None,
        None,
        move |args: &Bound<'_, PyTuple>, _kwargs: Option<&Bound<'_, PyDict>>| -> PyResult<Py<PyAny>> {
            let py = args.py();
            let run_connection = connection.clone_ref(py);
            let run_execute = execute.clone_ref(py);
            let run_sql = query.sql.clone();
            let run = PyCFunction::new_closure(
                py,
                None,
                None,
                move |args: &Bound<'_, PyTuple>, _kwargs: Option<&Bound<'_, PyDict>>| -> PyResult<Py<PyAny>> {
                    let py = args.py();
                    Ok(run_execute
                        .bind(py)
                        .call((), Some(&keywords(py, &[
                            ("connection", run_connection.bind(py).clone()),
                            ("sql", text(py, &run_sql)),
                        ])?))?
                        .call_method0("fetchall")?
                        .unbind())
                },
            )?;
            let rows = call_with(
                &host_entry(py, RUN_RECORDED_QUERY)?,
                &[("sql", text(py, &query.sql)), ("run", run.into_any())],
            );
            match rows {
                Ok(rows) => Ok(rows.unbind()),
                Err(error) if error.is_instance_of::<PyException>(py) => {
                    let wrapped: PyErr = planner_error(
                        py,
                        &format!(
                            "model '{}': failed to query highest eligible target cursor for {}.{}: {}",
                            query.model_name,
                            query.target,
                            query.cursor_column,
                            error.value(py).str()?
                        ),
                        (None, None),
                    );
                    wrapped.set_cause(py, Some(error));
                    Err(wrapped)
                }
                Err(error) => Err(error),
            }
        },
    )?
    .into_any())
}

pub(crate) fn run_bounded<'py>(
    py: Python<'py>,
    tasks: Vec<Bound<'py, PyAny>>,
    concurrency: i64,
    on_complete: Option<Bound<'py, PyAny>>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let mut fields: Vec<(&str, Bound<'py, PyAny>)> = vec![
        ("tasks", PyTuple::new(py, tasks)?.into_any()),
        ("concurrency", concurrency.into_pyobject(py)?.into_any()),
    ];
    if let Some(on_complete) = on_complete {
        fields.push(("on_complete", on_complete));
    }
    call_with(&host_entry(py, RUN_BOUNDED)?, &fields)?
        .try_iter()?
        .collect()
}
