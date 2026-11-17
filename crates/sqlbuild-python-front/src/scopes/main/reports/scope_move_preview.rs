//! `preview_scope_move`: visibility deltas of a resource move, without mutating the lookup.

use crate::scopes::_helpers::reports::scope_reports;
use crate::scopes::_helpers::reports::scope_reports::PyObject;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// `preview_scope_move`: visibility deltas of a resource move, without mutating the lookup.
#[inline]
pub fn scope_move_preview(
    py: Python<'_>,
    lookup: &Bound<'_, PyAny>,
    resource: &Bound<'_, PyAny>,
    destination: &Bound<'_, PyAny>,
) -> PyResult<(Option<PyObject>, PyObject)> {
    scope_reports::scope_move_preview(py, lookup, resource, destination)
}
