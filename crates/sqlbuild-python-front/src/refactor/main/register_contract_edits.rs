//! Register the contract edits functions and classes.

use crate::refactor::main::edit_contract_model::edit_contract_model;
use crate::refactor::main::edit_contract_source::edit_contract_source;
use crate::refactor::main::remaining_contract_findings::remaining_contract_findings;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the contract edits functions and classes.
pub fn register_contract_edits(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(edit_contract_model, module)?)?;
    module.add_function(wrap_pyfunction!(edit_contract_source, module)?)?;
    module.add_function(wrap_pyfunction!(remaining_contract_findings, module)?)?;
    Ok(())
}
