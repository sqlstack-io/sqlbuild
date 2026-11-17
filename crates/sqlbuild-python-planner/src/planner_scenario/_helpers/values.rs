//! Existing Python model constructors and scenario diagnostics.

use pyo3::prelude::{Bound, PyAny, PyErr, PyResult, Python};
use pyo3::types::PyString;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::types::ErrorId;

pub(crate) fn string<'py>(py: Python<'py>, text: &str) -> Bound<'py, PyAny> {
    PyString::new(py, text).into_any()
}

pub(crate) fn error(
    py: Python<'_>,
    message: &str,
    code: &str,
    help: Option<&str>,
) -> PyResult<PyErr> {
    let mut args = vec![("message", string(py, message)), ("code", string(py, code))];
    if let Some(help) = help {
        args.push(("help", string(py, help)));
    }
    Ok(PyErr::from_value(call_with(
        &error_class(py, ErrorId::PlannerInput)?,
        &args,
    )?))
}
