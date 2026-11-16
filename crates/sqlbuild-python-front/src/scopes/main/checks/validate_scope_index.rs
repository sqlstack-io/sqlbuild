//! Python's `validate_scope_index`: raise one `ScopeValidationError` for its error diagnostics.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;

/// Python's `validate_scope_index`: raise one `ScopeValidationError` for its error diagnostics.
#[pyfunction]
#[pyo3(signature = (*, index))]
pub fn validate_scope_index(index: &Bound<'_, PyAny>) -> Result<(), PyErr> {
    let py: Python<'_> = index.py();
    let error: Bound<'_, PyAny> = enum_member(py, RecordId::ScopeDiagnosticSeverity, "ERROR")?;
    let mut errors: Vec<Bound<'_, PyAny>> = Vec::new();
    for item in native_field(index, "diagnostics")?.try_iter()? {
        let item: Bound<'_, PyAny> = item?;
        if native_field(&item, "severity")?.is(&error) {
            errors.push(item);
        }
    }
    if errors.is_empty() {
        return Ok(());
    }
    let mut message: String = String::from("Invalid declaration scope index:");
    for item in &errors {
        let code: String = native_field(&native_field(item, "code")?, "value")?.extract()?;
        let text: String = native_field(item, "message")?.extract()?;
        message.push_str(&format!("\n[{code}] {text}"));
    }
    let options: Bound<'_, PyDict> = PyDict::new(py);
    options.set_item("diagnostics", PyTuple::new(py, errors)?)?;
    Err(PyErr::from_value(
        error_class(py, ErrorId::ScopeValidation)?.call((message,), Some(&options))?,
    ))
}
