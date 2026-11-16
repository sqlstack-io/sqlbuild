//! Direct `#[getter]` reads of this module's native pyclasses, installed into `records::getters`.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};

use sqlbuild_python_project::analysis::models::BindingCatalog;

pub(crate) fn read_binding_catalog<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<BindingCatalog>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "schemas" => (&this.schemas).into_bound_py_any(py),
        "dialect" => (&this.dialect).into_bound_py_any(py),
        "quoted_ignore_case" => (&this.quoted_ignore_case).into_bound_py_any(py),
        "analysis_shapes" => (&this.analysis_shapes).into_bound_py_any(py),
        "expression_shapes" => (&this.expression_shapes).into_bound_py_any(py),
        "native" => (&this.native).into_bound_py_any(py),
        _ => return None,
    })
}
