//! Direct `#[getter]` reads of this module's native pyclasses, installed into `records::getters`.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};

use crate::bindings::_helpers::build_progress::build_callbacks::BuildProgressCallbacks;
use sqlbuild_python_base::build_progress::models::TransientLineCoordinator;
use sqlbuild_python_base::records::main::registry::class_of::class_of;
use sqlbuild_python_base::records::models::GetterEntry;

/// The getter readers of this module's native pyclasses.
pub(crate) const GETTERS: &[GetterEntry] = &[
    GetterEntry {
        class: class_of::<BuildProgressCallbacks>,
        read: read_build_progress_callbacks,
    },
    GetterEntry {
        class: class_of::<TransientLineCoordinator>,
        read: read_transient_line_coordinator,
    },
];

fn read_build_progress_callbacks<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<BuildProgressCallbacks>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "elapsed" => this.elapsed().into_bound_py_any(py),
        _ => return None,
    })
}

fn read_transient_line_coordinator<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<TransientLineCoordinator>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "lock" => (&this.lock).into_bound_py_any(py),
        _ => return None,
    })
}
