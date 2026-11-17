//! The tolerant index: fault diagnostics, prospective resources and partial completeness.

use crate::scopes::_helpers::checks::scope_tolerant;
use pyo3::prelude::{Bound, Py, PyAny, PyResult, Python};

/// The tolerant index: fault diagnostics, prospective resources and partial completeness.
#[inline]
pub fn scope_tolerant_index(
    py: Python<'_>,
    index: &Bound<'_, PyAny>,
    snapshot: &Bound<'_, PyAny>,
    faults: (Bound<'_, PyAny>, Bound<'_, PyAny>),
) -> PyResult<Py<PyAny>> {
    scope_tolerant::scope_tolerant_index(py, index, snapshot, faults)
}
