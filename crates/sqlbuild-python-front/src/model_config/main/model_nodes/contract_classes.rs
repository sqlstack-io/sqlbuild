//! The contract classes native header parsing builds columns and audits as.

use crate::model_config::_helpers::model_nodes::contract_classes;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyDict;

/// The contract classes native header parsing builds columns and audits as.
#[inline]
pub fn contract_classes(py: Python<'_>) -> PyResult<Bound<'_, PyDict>> {
    contract_classes::contract_classes(py)
}
