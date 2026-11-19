//! Direct `#[getter]` reads of this module's native pyclasses, installed into `records::getters`.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};

use crate::dbt_events::models::DbtEventStreamUpdate;

/// The getter values of a `DbtEventStreamUpdate`.
pub(crate) fn read_dbt_event_stream_update<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<DbtEventStreamUpdate>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "status_message" => (&this.status_message).into_bound_py_any(py),
        "result" => (&this.result).into_bound_py_any(py),
        "row" => (&this.row).into_bound_py_any(py),
        "finished" => (&this.finished).into_bound_py_any(py),
        _ => return None,
    })
}
