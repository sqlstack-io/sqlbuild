//! A `CompileInputError` carrying `message`.

use pyo3::Python;
use pyo3::prelude::{PyAnyMethods, PyErr, PyResult};
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::types::ErrorId;

/// A `CompileInputError` carrying `message`.
pub fn compile_input_error(py: Python<'_>, message: String) -> PyResult<PyErr> {
    Ok(PyErr::from_value(
        error_class(py, ErrorId::CompileInput)?.call1((message,))?,
    ))
}
