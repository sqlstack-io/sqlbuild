//! The verified cached `ScopeIndex` at `path` for `fingerprint`; every fault is a miss.

use crate::scopes::_helpers::index::scope_objects::index_object;
use crate::scopes::models::ScopeClasses;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Py, PyAny, PyResult, Python};
use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::types::CompilerDetach;
use sqlbuild_scopes::scope_cache::main::read_scope_cache::read_scope_cache;
use std::path::PathBuf;

/// The verified cached `ScopeIndex` at `path` for `fingerprint`; every fault is a miss.
pub fn read_scope_index_cache(
    py: Python<'_>,
    path: PathBuf,
    fingerprint: String,
) -> PyResult<Option<Py<PyAny>>> {
    let cached = py
        .compiler_detach(|| Ok(read_scope_cache(&path, &fingerprint)))
        .map_err(compiler_error)?;
    let Ok(index) = cached else {
        return Ok(None);
    };
    match index_object(&ScopeClasses::new(py)?, &index) {
        Ok(object) => Ok(Some(object.unbind())),
        Err(error) if error.is_instance_of::<PyValueError>(py) => Ok(None),
        Err(error) => Err(error),
    }
}
