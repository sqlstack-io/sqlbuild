//! Open one cache kind's store with the GIL released; a failed read is an `OSError`.

use crate::cache::_helpers::native_store;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use sqlbuild_cache::store::models::NativeStore;
use std::path::Path;

/// Open one cache kind's store with the GIL released; a failed read is an `OSError`.
#[inline]
pub fn open_store(
    py: Python<'_>,
    path: &Path,
    kind: &str,
    environment: &str,
) -> PyResult<NativeStore> {
    native_store::open_store(py, path, kind, environment)
}
