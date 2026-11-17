//! Direct `#[getter]` reads of this module's native pyclasses, installed into `records::getters`.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};

use crate::compute_logs::_helpers::storage::LocalFilesystemComputeLogStorage;

pub(crate) fn read_local_filesystem_compute_log_storage<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<LocalFilesystemComputeLogStorage>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "root" => this.root(py).and_then(|value| value.into_bound_py_any(py)),
        _ => return None,
    })
}
