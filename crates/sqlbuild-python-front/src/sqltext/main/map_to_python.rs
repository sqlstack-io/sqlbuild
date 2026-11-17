//! Map to python.

use crate::sqltext::main::authored_value_to_python::authored_value_to_python;
use pyo3::prelude::{Py, PyAny, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_sqltext::compiler::models::AuthoredValue;

/// Map to python.
pub fn map_to_python(py: Python<'_>, values: &[(String, AuthoredValue)]) -> PyResult<Py<PyAny>> {
    let result = PyDict::new(py);
    for (key, value) in values {
        result.set_item(key, authored_value_to_python(py, value)?)?;
    }
    Ok(result.unbind().into_any())
}
