//! Direct `#[getter]` reads of this module's native pyclasses, installed into `records::getters`.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};

use crate::execution_history::models::PostgresExecutionHistory;
use crate::execution_history::models::SqliteExecutionHistory;

pub(crate) fn read_postgres_execution_history<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<PostgresExecutionHistory>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "project_calls" => this.project_calls().into_bound_py_any(py),
        _ => return None,
    })
}

pub(crate) fn read_sqlite_execution_history<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<SqliteExecutionHistory>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "path" => this.path(py).and_then(|value| value.into_bound_py_any(py)),
        "project_calls" => this.project_calls().into_bound_py_any(py),
        _ => return None,
    })
}
