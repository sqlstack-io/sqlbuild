//! Direct `#[getter]` reads of this module's native pyclasses, installed into `records::getters`.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};

use crate::bindings::_helpers::dbt_events::stream::DbtEventStreamUpdate;
use sqlbuild_python_base::records::main::registry::class_of::class_of;
use sqlbuild_python_base::records::models::GetterEntry;

/// The getter readers of this module's native pyclasses.
pub(crate) const GETTERS: &[GetterEntry] = &[GetterEntry {
    class: class_of::<DbtEventStreamUpdate>,
    read: read_dbt_event_stream_update,
}];

fn read_dbt_event_stream_update<'py>(
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
