//! `values.get(key)` as its Python object, `None` when absent.

use crate::model_config::_helpers::config::typed_config;
use crate::model_config::types::ConfigValues;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// `values.get(key)` as its Python object, `None` when absent.
#[inline]
pub fn config_value<'py>(
    py: Python<'py>,
    values: &ConfigValues<'_>,
    key: &str,
) -> PyResult<Bound<'py, PyAny>> {
    typed_config::config_value(py, values, key)
}
