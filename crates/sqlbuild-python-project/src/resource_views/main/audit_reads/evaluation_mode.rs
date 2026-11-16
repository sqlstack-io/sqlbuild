//! The compiled audit's `evaluation_mode`, as its Python attribute reads it.

use crate::resource_views::_helpers::audit_reads::native_audit;
use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The compiled audit's `evaluation_mode`, as its Python attribute reads it.
pub fn evaluation_mode<'py>(audit: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = audit.py();
    if let Some(view) = native_audit(audit)? {
        return view
            .evaluation_mode(py)
            .and_then(|value| value.into_bound_py_any(py));
    }
    native_field(audit, "evaluation_mode")
}
