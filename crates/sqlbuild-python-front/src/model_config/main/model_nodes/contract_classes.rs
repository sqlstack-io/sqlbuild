//! The contract classes native header parsing builds columns and audits as.

use crate::model_config::_helpers::model_nodes::contract_classes::CONTRACT_CLASS_TABLE;
use crate::model_config::_helpers::model_nodes::contract_classes::built_contract_classes;
use pyo3::prelude::{Bound, PyResult, Python};
use pyo3::types::PyDict;

/// The contract classes native header parsing builds columns and audits as.
pub fn contract_classes(py: Python<'_>) -> PyResult<Bound<'_, PyDict>> {
    CONTRACT_CLASS_TABLE
        .get_or_try_init(py, || built_contract_classes(py).map(Bound::unbind))
        .map(|classes| classes.bind(py).clone())
}
