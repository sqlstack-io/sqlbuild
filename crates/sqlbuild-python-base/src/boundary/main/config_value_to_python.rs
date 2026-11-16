//! Build the Python object for one loaded value; mapping entries keep their document order.

use crate::boundary::_helpers::config_values;
use pyo3::prelude::PyResult;
use pyo3::prelude::{Py, PyAny, Python};
use sqlbuild_config::models::ConfigValue;

/// Build the Python object for one loaded value; mapping entries keep their document order.
#[inline]
pub fn config_value_to_python(py: Python<'_>, value: ConfigValue) -> PyResult<Py<PyAny>> {
    config_values::config_value_to_python(py, value)
}
