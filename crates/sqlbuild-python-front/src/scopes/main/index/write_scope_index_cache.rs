//! Atomically write one complete `ScopeIndex` for `fingerprint`; whether the write landed.

use crate::scopes::_helpers::index::scope_cache;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use std::path::PathBuf;

/// Atomically write one complete `ScopeIndex` for `fingerprint`; whether the write landed.
#[inline]
pub fn write_scope_index_cache(
    py: Python<'_>,
    location: (PathBuf, String),
    fingerprint: String,
    index: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    scope_cache::write_scope_index_cache(py, location, fingerprint, index)
}
