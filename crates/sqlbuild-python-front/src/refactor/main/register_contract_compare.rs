//! Register the contract compare functions and classes.

use crate::refactor::_helpers::contract_compare::compare_contract_evidence;
use crate::refactor::_helpers::contract_compare::compare_contracts;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the contract compare functions and classes.
pub fn register_contract_compare(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(compare_contracts, module)?)?;
    module.add_function(wrap_pyfunction!(compare_contract_evidence, module)?)?;
    Ok(())
}
