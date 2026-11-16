//! Optional authored value to python.

use crate::sqltext::_helpers::authored_values;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use sqlbuild_sqltext::compiler::models::AuthoredValue;

/// Optional authored value to python.
#[inline]
pub fn optional_authored_value_to_python(
    py: Python<'_>,
    value: Option<&AuthoredValue>,
) -> PyResult<Option<Py<PyAny>>> {
    authored_values::optional_authored_value_to_python(py, value)
}
