//! Selected declarations compared with one bulk read of physical columns.

use crate::refactor::_helpers::contract_compare;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;
use pyo3::types::PyTuple;

/// Selected declarations compared with one bulk read of physical columns.
#[inline]
pub fn compare_contracts<'py>(
    request: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    contract_compare::compare_contracts(request)
}
