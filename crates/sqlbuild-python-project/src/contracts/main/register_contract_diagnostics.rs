//! Register the contract diagnostics functions and classes.

use crate::contracts::_helpers::contract_diagnostics::evaluate_model_contracts;
use crate::contracts::main::promotion_conflict_diagnostics::promotion_conflict_diagnostics;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the contract diagnostics functions and classes.
pub fn register_contract_diagnostics(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(evaluate_model_contracts, module)?)?;
    module.add_function(wrap_pyfunction!(promotion_conflict_diagnostics, module)?)?;
    Ok(())
}
