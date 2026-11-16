//! Carry an uninspected value: exact containers keep their own objects without opening items.

use crate::model_config::_helpers::config::config_nodes;
use crate::model_config::types::PyConfigNode;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Carry an uninspected value: exact containers keep their own objects without opening items.
#[inline]
pub fn carried_node(value: &Bound<'_, PyAny>) -> PyResult<PyConfigNode> {
    config_nodes::carried_node(value)
}
