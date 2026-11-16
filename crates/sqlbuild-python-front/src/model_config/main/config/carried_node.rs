//! Carry an uninspected value: exact containers keep their own objects without opening items.

use crate::model_config::_helpers::config::config_nodes::host;
use crate::model_config::_helpers::config::config_nodes::is_exact_container;
use crate::model_config::main::config::config_node::config_node;
use crate::model_config::types::PyConfigNode;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_model_config::config_values::models::HostChildren;

/// Carry an uninspected value: exact containers keep their own objects without opening items.
pub fn carried_node(value: &Bound<'_, PyAny>) -> PyResult<PyConfigNode> {
    if is_exact_container(value) {
        host(value, HostChildren::None)
    } else {
        config_node(value)
    }
}
