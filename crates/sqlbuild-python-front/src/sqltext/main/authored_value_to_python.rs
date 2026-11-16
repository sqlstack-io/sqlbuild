//! One authored value as the Python header object discovery hands to config builds.

use crate::sqltext::_helpers::authored_values;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use sqlbuild_sqltext::compiler::models::AuthoredValue;

/// One authored value as the Python header object discovery hands to config builds.
#[inline]
pub fn authored_value_to_python(py: Python<'_>, value: &AuthoredValue) -> PyResult<Py<PyAny>> {
    authored_values::authored_value_to_python(py, value)
}
