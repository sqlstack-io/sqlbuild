//! `query_scope_report`: one existing target or prospective path; invalid paths raise.

use crate::scopes::_helpers::reports::scope_reports;
use crate::scopes::_helpers::reports::scope_reports::PyObject;
use crate::scopes::models::ReportArguments;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// `query_scope_report`: one existing target or prospective path; invalid paths raise.
#[inline]
pub fn scope_report(
    py: Python<'_>,
    lookup: &Bound<'_, PyAny>,
    request: ReportArguments<'_>,
) -> PyResult<PyObject> {
    scope_reports::scope_report(py, lookup, request)
}
