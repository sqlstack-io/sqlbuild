//! Return P005, P007 and P008 errors and unmatched Python-SQL relations; raise P001.

use crate::references::_helpers::explicit_references;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};

/// Return P005, P007 and P008 errors and unmatched Python-SQL relations; raise P001.
#[inline]
pub fn explicit_reference_report<'py>(
    py: Python<'py>,
    project: &Bound<'py, PyAny>,
    discovered_inputs: &Bound<'py, PyAny>,
    enforce_explicit: bool,
) -> PyResult<Bound<'py, PyAny>> {
    explicit_references::explicit_reference_report(py, project, discovered_inputs, enforce_explicit)
}
