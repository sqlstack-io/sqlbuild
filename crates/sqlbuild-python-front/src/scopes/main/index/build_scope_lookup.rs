//! Python's `build_scope_lookup`: the canonical index, its mappings and the native lookup.

use crate::scopes::_helpers::index::scope_lookup;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// Python's `build_scope_lookup`: the canonical index, its mappings and the native lookup.
#[inline]
pub fn build_scope_lookup(py: Python<'_>, index: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    scope_lookup::build_scope_lookup(py, index)
}
