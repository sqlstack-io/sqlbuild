//! Direct `#[getter]` reads of this module's native pyclasses, installed into `records::getters`.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};

use crate::bindings::_helpers::execution_history::postgres::PostgresExecutionHistory;
use crate::bindings::_helpers::execution_history::sqlite::SqliteExecutionHistory;
use sqlbuild_python_base::records::main::registry::class_of::class_of;
use sqlbuild_python_base::records::models::GetterEntry;

/// The getter readers of this module's native pyclasses.
pub(crate) const GETTERS: &[GetterEntry] = &[
    GetterEntry {
        class: class_of::<PostgresExecutionHistory>,
        read: read_postgres_execution_history,
    },
    GetterEntry {
        class: class_of::<SqliteExecutionHistory>,
        read: read_sqlite_execution_history,
    },
];

fn read_postgres_execution_history<'py>(
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

fn read_sqlite_execution_history<'py>(
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
