//! `values.get(key)` as its typed node, `None` (null) when absent.

use crate::model_config::_helpers::config::typed_config;
use crate::model_config::types::ConfigValues;
use crate::model_config::types::PyConfigNode;
use pyo3::prelude::PyResult;
use std::sync::Arc;

/// `values.get(key)` as its typed node, `None` (null) when absent.
#[inline]
pub fn config_node_of(values: &ConfigValues<'_>, key: &str) -> PyResult<Arc<PyConfigNode>> {
    typed_config::config_node_of(values, key)
}
