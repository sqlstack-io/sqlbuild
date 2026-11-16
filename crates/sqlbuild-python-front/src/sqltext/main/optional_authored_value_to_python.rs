//! Optional authored value to python.

use crate::sqltext::main::authored_value_to_python::authored_value_to_python;
use pyo3::prelude::{Py, PyAny, PyResult, Python};
use sqlbuild_sqltext::compiler::models::AuthoredValue;

/// Optional authored value to python.
pub fn optional_authored_value_to_python(
    py: Python<'_>,
    value: Option<&AuthoredValue>,
) -> PyResult<Option<Py<PyAny>>> {
    value
        .map(|item| authored_value_to_python(py, item))
        .transpose()
}
