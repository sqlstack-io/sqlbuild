//! The stable code, message and help of an expected exception-like object.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult};
use pyo3::types::PyString;
use pyo3::{pyfunction, wrap_pyfunction};

/// Return a stable code from an expected exception-like object.
#[pyfunction]
#[pyo3(signature = (*, error, fallback_code))]
fn error_code(error: &Bound<'_, PyAny>, fallback_code: &Bound<'_, PyAny>) -> PyResult<String> {
    match error.getattr_opt("code")? {
        Some(code) => Ok(code.str()?.to_string()),
        None => Ok(fallback_code.str()?.to_string()),
    }
}

/// Return expected-error help text when present.
#[pyfunction]
fn error_help<'py>(error: &Bound<'py, PyAny>) -> PyResult<Option<Bound<'py, PyAny>>> {
    Ok(error
        .getattr_opt("help")?
        .filter(|help| help.is_instance_of::<PyString>()))
}

/// Return a stable message from an expected exception-like object.
#[pyfunction]
fn error_message(error: &Bound<'_, PyAny>) -> PyResult<String> {
    match error.getattr_opt("message")? {
        Some(message) => Ok(message.str()?.to_string()),
        None => Ok(error.str()?.to_string()),
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(error_code, module)?)?;
    module.add_function(wrap_pyfunction!(error_help, module)?)?;
    module.add_function(wrap_pyfunction!(error_message, module)?)
}
