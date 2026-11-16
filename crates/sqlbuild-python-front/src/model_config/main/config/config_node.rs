//! One Python config value as a `ConfigNode`; a value that holds itself stays opaque whole.

use crate::model_config::_helpers::config::config_nodes::convert;
use crate::model_config::_helpers::config::config_nodes::host;
use crate::model_config::types::PyConfigNode;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_model_config::config_values::models::HostChildren;

/// One Python config value as a `ConfigNode`; a value that holds itself stays opaque whole.
pub fn config_node(value: &Bound<'_, PyAny>) -> PyResult<PyConfigNode> {
    match convert(value, &[])? {
        Some(node) => Ok(node),
        None => host(value, HostChildren::None),
    }
}
