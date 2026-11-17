//! `values.get(key)` as its Python object, `None` when absent.

use crate::model_config::_helpers::config::typed_config::node;
use crate::model_config::main::config::config_object::config_object;
use crate::model_config::types::ConfigValues;
use crate::model_config::types::PyConfigNode;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use std::sync::Arc;

/// `values.get(key)` as its Python object, `None` when absent.
pub fn config_value<'py>(
    py: Python<'py>,
    values: &ConfigValues<'_>,
    key: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let value: Arc<PyConfigNode> = node(values, key)?;
    config_object(py, &value)
}
