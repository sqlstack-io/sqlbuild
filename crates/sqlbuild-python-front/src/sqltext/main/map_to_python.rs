//! Map to python.

use crate::sqltext::_helpers::authored_values;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use sqlbuild_sqltext::compiler::models::AuthoredValue;

/// Map to python.
#[inline]
pub fn map_to_python(py: Python<'_>, values: &[(String, AuthoredValue)]) -> PyResult<Py<PyAny>> {
    authored_values::map_to_python(py, values)
}
