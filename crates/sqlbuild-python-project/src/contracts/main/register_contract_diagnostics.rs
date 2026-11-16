//! Register the contract diagnostics functions and classes.

use crate::contracts::_helpers::contract_diagnostics;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the contract diagnostics functions and classes.
#[inline]
pub fn register_contract_diagnostics(module: &Bound<'_, PyModule>) -> PyResult<()> {
    contract_diagnostics::register(module)
}
