//! One K011 error per enforced-contract table model under immediate promotion.

use crate::contracts::_helpers::contract_diagnostics;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyTuple;

/// One K011 error per enforced-contract table model under immediate promotion.
#[inline]
pub fn promotion_conflict_diagnostics<'py>(
    py: Python<'py>,
    project: &Bound<'py, PyAny>,
    adapter_default: &Bound<'py, PyAny>,
    settings_file: String,
) -> PyResult<Bound<'py, PyTuple>> {
    contract_diagnostics::promotion_conflict_diagnostics(
        py,
        project,
        adapter_default,
        settings_file,
    )
}
