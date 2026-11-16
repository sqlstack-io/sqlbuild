//! Register the contract compare functions and classes.

use crate::refactor::_helpers::contract_compare;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the contract compare functions and classes.
#[inline]
pub fn register_contract_compare(module: &Bound<'_, PyModule>) -> PyResult<()> {
    contract_compare::register(module)
}
