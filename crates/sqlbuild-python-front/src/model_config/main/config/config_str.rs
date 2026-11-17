//! An authored string, preserving a present empty string.

use crate::model_config::_helpers::config::config_values;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// An authored string, preserving a present empty string.
#[inline]
pub fn config_str<'py>(
    values: &Bound<'py, PyAny>,
    key: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    config_values::config_str(values, key)
}
