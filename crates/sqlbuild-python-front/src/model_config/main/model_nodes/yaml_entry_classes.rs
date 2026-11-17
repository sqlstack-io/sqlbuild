//! The classes parsed seed and source declarations become.

use crate::model_config::_helpers::model_nodes::contract_classes::YAML_ENTRY_CLASS_TABLE;
use crate::model_config::_helpers::model_nodes::contract_classes::built_yaml_entry_classes;
use pyo3::prelude::{Bound, PyResult, Python};
use pyo3::types::PyDict;

/// The classes parsed seed and source declarations become.
pub fn yaml_entry_classes(py: Python<'_>) -> PyResult<Bound<'_, PyDict>> {
    YAML_ENTRY_CLASS_TABLE
        .get_or_try_init(py, || built_yaml_entry_classes(py).map(Bound::unbind))
        .map(|classes| classes.bind(py).clone())
}
