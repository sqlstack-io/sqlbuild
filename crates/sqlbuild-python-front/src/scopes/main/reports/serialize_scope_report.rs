//! `serialize_scope_report`: deterministic schema-version-two ASCII JSON of one result.

use crate::scopes::_helpers::reports::scope_reports::scope_error;
use crate::scopes::_helpers::reports::scope_reports::serialized;
use crate::scopes::models::ScopeClasses;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::pyfunction;
use sqlbuild_python_base::records::types::ErrorId;

/// `serialize_scope_report`: deterministic schema-version-two ASCII JSON of one result.
#[pyfunction]
#[pyo3(signature = (*, report))]
pub fn serialize_scope_report(py: Python<'_>, report: &Bound<'_, PyAny>) -> PyResult<String> {
    let classes: ScopeClasses<'_> = ScopeClasses::new(py)?;
    match serialized(&classes, report) {
        Err(error) if error.is_instance_of::<PyValueError>(py) => {
            let message: String = error.value(py).str()?.extract()?;
            let raised: PyErr = scope_error(py, ErrorId::Scope, message)?;
            raised.set_cause(py, Some(error));
            Err(raised)
        }
        other => other,
    }
}
