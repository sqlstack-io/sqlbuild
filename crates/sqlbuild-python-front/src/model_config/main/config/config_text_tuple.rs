//! `key`'s strings as a tuple, or `None` when absent or `None`.

use crate::model_config::_helpers::config::typed_config;
use crate::model_config::types::ConfigValues;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyTuple;

/// `key`'s strings as a tuple, or `None` when absent or `None`.
#[inline]
pub fn config_text_tuple<'py>(
    py: Python<'py>,
    values: &ConfigValues<'_>,
    key: &str,
) -> PyResult<Option<Bound<'py, PyTuple>>> {
    typed_config::config_text_tuple(py, values, key)
}
