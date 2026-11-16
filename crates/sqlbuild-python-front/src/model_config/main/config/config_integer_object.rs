//! `key`'s integer value as its Python object, or `None` when absent or `None`.

use crate::model_config::_helpers::config::typed_config;
use crate::model_config::types::ConfigValues;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// `key`'s integer value as its Python object, or `None` when absent or `None`.
#[inline]
pub fn config_integer_object<'py>(
    py: Python<'py>,
    values: &ConfigValues<'_>,
    key: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    typed_config::config_integer_object(py, values, key)
}
