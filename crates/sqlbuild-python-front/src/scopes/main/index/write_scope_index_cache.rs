//! Atomically write one complete `ScopeIndex` for `fingerprint`; whether the write landed.

use crate::scopes::_helpers::index::scope_objects::{read_completeness, read_index};
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::types::CompilerDetach;
use sqlbuild_scopes::scope_cache::main::encode_scope_cache::encode_scope_cache;
use sqlbuild_scopes::scope_cache::main::write_scope_cache::write_scope_cache;
use sqlbuild_scopes::scope_cache::models::CachedIndex;
use std::path::PathBuf;

/// Atomically write one complete `ScopeIndex` for `fingerprint`; whether the write landed.
pub fn write_scope_index_cache(
    py: Python<'_>,
    location: (PathBuf, String),
    fingerprint: String,
    index: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    if !read_completeness(index)?.iter().all(|done| *done) {
        return Ok(false);
    }
    let index: CachedIndex = read_index(index)?;
    let (directory, file_name) = location;
    py.compiler_detach(|| {
        Ok(match encode_scope_cache(&index, &fingerprint) {
            Ok(envelope) => matches!(write_scope_cache(&directory, &file_name, &envelope), Ok(())),
            Err(_) => false,
        })
    })
    .map_err(compiler_error)
}
