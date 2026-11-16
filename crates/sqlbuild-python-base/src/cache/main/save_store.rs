//! Atomically save a store holding unsaved values with the GIL released; returns entries written.

use crate::cache::_helpers::native_store;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use sqlbuild_cache::store::models::NativeStore;
use std::path::Path;

/// Atomically save a store holding unsaved values with the GIL released; returns entries written.
#[inline]
pub fn save_store(
    py: Python<'_>,
    store: &NativeStore,
    path: &Path,
    metadata: &[u8],
) -> PyResult<Option<usize>> {
    native_store::save_store(py, store, path, metadata)
}
