//! The findings an edit leaves, plus an ownership-conflict finding when the edit was refused.

use crate::refactor::_helpers::contract_edits;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyTuple;

/// The findings an edit leaves, plus an ownership-conflict finding when the edit was refused.
#[inline]
pub fn remaining_contract_findings<'py>(
    py: Python<'py>,
    findings: &Bound<'py, PyAny>,
    overwrite: bool,
    conflict: Option<String>,
) -> PyResult<Bound<'py, PyTuple>> {
    contract_edits::remaining_contract_findings(py, findings, overwrite, conflict)
}
