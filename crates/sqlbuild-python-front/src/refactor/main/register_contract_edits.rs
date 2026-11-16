//! Register the contract edits functions and classes.

use crate::refactor::_helpers::contract_edits;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the contract edits functions and classes.
#[inline]
pub fn register_contract_edits(module: &Bound<'_, PyModule>) -> PyResult<()> {
    contract_edits::register(module)
}
