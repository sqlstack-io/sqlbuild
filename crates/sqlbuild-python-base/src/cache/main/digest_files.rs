//! Hexadecimal content digest of each file in `paths`, or `None` where it cannot be read.

use crate::cache::_helpers::native_store;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use std::path::PathBuf;

/// Hexadecimal content digest of each file in `paths`, or `None` where it cannot be read.
#[inline]
pub fn digest_files(py: Python<'_>, paths: Vec<PathBuf>) -> PyResult<Vec<Option<String>>> {
    native_store::digest_files(py, paths)
}
