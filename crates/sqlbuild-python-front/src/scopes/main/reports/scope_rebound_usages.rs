//! Scoped declarations the resources at `path` use that resolve differently from `destination`.

use crate::scopes::_helpers::reports::scope_reports;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// Scoped declarations the resources at `path` use that resolve differently from `destination`.
#[inline]
pub fn scope_rebound_usages(
    py: Python<'_>,
    lookup: &Bound<'_, PyAny>,
    paths: (&str, &str),
) -> PyResult<Option<Vec<String>>> {
    scope_reports::scope_rebound_usages(py, lookup, paths)
}
