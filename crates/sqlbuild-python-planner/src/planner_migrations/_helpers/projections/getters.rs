//! Direct `#[getter]` reads of this module's native pyclasses, installed into `records::getters`.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};

use crate::planner_migrations::models::MigrationStateInspection;

pub(crate) fn read_migration_state_inspection<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<MigrationStateInspection>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "database" => this.database(py).into_bound_py_any(py),
        "events" => this
            .events(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "column_events" => this
            .column_events(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "old_name_histories" => this
            .old_name_histories(py)
            .and_then(|value| value.into_bound_py_any(py)),
        _ => return None,
    })
}
