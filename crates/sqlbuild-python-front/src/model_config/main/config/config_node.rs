//! One Python config value as a `ConfigNode`; a value that holds itself stays opaque whole.

use crate::model_config::_helpers::config::config_nodes;
use crate::model_config::types::PyConfigNode;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// One Python config value as a `ConfigNode`; a value that holds itself stays opaque whole.
#[inline]
pub fn config_node(value: &Bound<'_, PyAny>) -> PyResult<PyConfigNode> {
    config_nodes::config_node(value)
}
