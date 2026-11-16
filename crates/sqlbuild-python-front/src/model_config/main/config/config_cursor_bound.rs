//! `key`'s cursor bound as text, or `None` when absent or `None`.

use crate::model_config::_helpers::config::typed_config;
use crate::model_config::types::ConfigValues;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// `key`'s cursor bound as text, or `None` when absent or `None`.
#[inline]
pub fn config_cursor_bound<'py>(
    py: Python<'py>,
    values: &ConfigValues<'_>,
    key: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    typed_config::config_cursor_bound(py, values, key)
}
