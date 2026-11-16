//! The verified cached `ScopeIndex` at `path` for `fingerprint`; every fault is a miss.

use crate::scopes::_helpers::index::scope_cache;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use std::path::PathBuf;

/// The verified cached `ScopeIndex` at `path` for `fingerprint`; every fault is a miss.
#[inline]
pub fn read_scope_index_cache(
    py: Python<'_>,
    path: PathBuf,
    fingerprint: String,
) -> PyResult<Option<Py<PyAny>>> {
    scope_cache::read_scope_index_cache(py, path, fingerprint)
}
