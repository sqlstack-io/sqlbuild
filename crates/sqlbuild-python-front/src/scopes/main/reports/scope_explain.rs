//! `explain_scope_declaration`: one qualified declaration in an optional context.

use crate::scopes::_helpers::reports::scope_reports;
use crate::scopes::_helpers::reports::scope_reports::PyObject;
use crate::scopes::models::ExplainArguments;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// `explain_scope_declaration`: one qualified declaration in an optional context.
#[inline]
pub fn scope_explain(
    py: Python<'_>,
    lookup: &Bound<'_, PyAny>,
    request: ExplainArguments<'_>,
) -> PyResult<(PyObject, PyObject)> {
    scope_reports::scope_explain(py, lookup, request)
}
