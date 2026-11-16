//! The native error `class(message, code=code)`.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::types::ErrorId;

/// The native error `class(message, code=code)`.
pub fn input_error(py: Python<'_>, class: ErrorId, message: &str, code: Option<&str>) -> PyErr {
    let raised: PyResult<Bound<'_, PyAny>> = error_class(py, class).and_then(|class| {
        let options: Bound<'_, PyDict> = PyDict::new(py);
        if let Some(code) = code {
            options.set_item("code", code)?;
        }
        class.call((message,), Some(&options))
    });
    match raised {
        Ok(error) => PyErr::from_value(error),
        Err(error) => error,
    }
}
