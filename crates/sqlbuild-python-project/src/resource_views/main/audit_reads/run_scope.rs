//! The compiled audit's `run_scope`, as its Python attribute reads it.

use crate::resource_views::_helpers::audit_reads::native_audit;
use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The compiled audit's `run_scope`, as its Python attribute reads it.
pub fn run_scope<'py>(audit: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = audit.py();
    if let Some(view) = native_audit(audit)? {
        return view
            .run_scope(py)
            .and_then(|value| value.into_bound_py_any(py));
    }
    native_field(audit, "run_scope")
}
