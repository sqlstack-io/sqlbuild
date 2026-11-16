//! Every model's column contract diagnostics in model order; no build is staged without `adapter_default`.

use crate::contracts::_helpers::contract_diagnostics;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};

/// Every model's column contract diagnostics in model order; no build is staged without `adapter_default`.
#[inline]
pub fn evaluate_model_contracts<'py>(
    py: Python<'py>,
    project: &Bound<'py, PyAny>,
    dialect: Option<&Bound<'py, PyAny>>,
    adapter_default: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    contract_diagnostics::evaluate_model_contracts(py, project, dialect, adapter_default)
}
