//! The classes parsed seed and source declarations become.

use crate::model_config::_helpers::model_nodes::contract_classes;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyDict;

/// The classes parsed seed and source declarations become.
#[inline]
pub fn yaml_entry_classes(py: Python<'_>) -> PyResult<Bound<'_, PyDict>> {
    contract_classes::yaml_entry_classes(py)
}
