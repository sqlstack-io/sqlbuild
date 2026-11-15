//! Direct `#[getter]` reads of this module's native pyclasses, installed into `records::getters`.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};

use crate::bindings::_helpers::presentation_text::style::PythonCliStyle;
use sqlbuild_python_base::records::main::registry::class_of::class_of;
use sqlbuild_python_base::records::models::GetterEntry;

/// The getter readers of this module's native pyclasses.
pub(crate) const GETTERS: &[GetterEntry] = &[GetterEntry {
    class: class_of::<PythonCliStyle>,
    read: read_python_cli_style,
}];

fn read_python_cli_style<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<PythonCliStyle>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "use_color" => (&this.use_color).into_bound_py_any(py),
        _ => return None,
    })
}
