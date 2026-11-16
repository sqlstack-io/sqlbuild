//! `browse_scope_folders`: the direct child declaration-definition folders of `folder`.

use crate::scopes::_helpers::reports::scope_reports;
use crate::scopes::_helpers::reports::scope_reports::PyObject;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// `browse_scope_folders`: the direct child declaration-definition folders of `folder`.
#[inline]
pub fn scope_browse(
    py: Python<'_>,
    lookup: &Bound<'_, PyAny>,
    request: (String, Option<Bound<'_, PyAny>>, bool),
) -> PyResult<PyObject> {
    scope_reports::scope_browse(py, lookup, request)
}
