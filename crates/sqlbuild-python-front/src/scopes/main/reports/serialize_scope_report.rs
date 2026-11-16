//! `serialize_scope_report`: deterministic schema-version-two ASCII JSON of one result.

use crate::scopes::_helpers::reports::scope_reports;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// `serialize_scope_report`: deterministic schema-version-two ASCII JSON of one result.
#[inline]
pub fn serialize_scope_report(py: Python<'_>, report: &Bound<'_, PyAny>) -> PyResult<String> {
    scope_reports::serialize_scope_report(py, report)
}
