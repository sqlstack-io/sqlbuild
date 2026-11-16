//! `list_scope_declarations`: declarations recursively beneath one definition folder.

use crate::scopes::_helpers::reports::scope_reports;
use crate::scopes::_helpers::reports::scope_reports::PyObject;
use crate::scopes::models::ListArguments;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// `list_scope_declarations`: declarations recursively beneath one definition folder.
#[inline]
pub fn scope_list(
    py: Python<'_>,
    lookup: &Bound<'_, PyAny>,
    request: ListArguments<'_>,
) -> PyResult<PyObject> {
    scope_reports::scope_list(py, lookup, request)
}
