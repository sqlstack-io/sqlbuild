//! Direct `#[getter]` reads of this module's native pyclasses, installed into `records::getters`.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};

use crate::scopes::models::NativeScopeIndex;

pub(crate) fn read_native_scope_index<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<NativeScopeIndex>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "index" => this.index(py).into_bound_py_any(py),
        "has_scoped_relationship_declarations" => this
            .has_scoped_relationship_declarations()
            .into_bound_py_any(py),
        _ => return None,
    })
}
