//! Direct `#[getter]` reads of this module's native pyclasses, installed into `records::getters`.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};

use crate::bindings::_helpers::model_config::errors::NativeConfigError;
use sqlbuild_python_base::records::main::registry::class_of::class_of;
use sqlbuild_python_base::records::models::GetterEntry;

/// The getter readers of this module's native pyclasses.
pub(crate) const GETTERS: &[GetterEntry] = &[GetterEntry {
    class: class_of::<NativeConfigError>,
    read: read_native_config_error,
}];

fn read_native_config_error<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<NativeConfigError>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "class_name" => (&this.class_name).into_bound_py_any(py),
        "message" => (&this.message).into_bound_py_any(py),
        "code" => (&this.code).into_bound_py_any(py),
        "help" => (&this.help).into_bound_py_any(py),
        "key" => (&this.key).into_bound_py_any(py),
        _ => return None,
    })
}
