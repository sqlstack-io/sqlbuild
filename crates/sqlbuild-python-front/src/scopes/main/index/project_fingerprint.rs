//! The scope index fingerprint of `project_dir`, `None` when it cannot be read.

use crate::scopes::_helpers::index::scope_cache::sqlbuild_version;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::{PyResult, Python};
use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::types::CompilerDetach;
use sqlbuild_scopes::scope_cache::main::project_fingerprint_inputs::project_fingerprint_inputs;
use sqlbuild_scopes::scope_cache::main::scope_fingerprint::scope_fingerprint;
use sqlbuild_scopes::scope_cache::models::{Fingerprint, FingerprintInputs};
use std::path::PathBuf;

/// The scope index fingerprint of `project_dir`, `None` when it cannot be read.
pub fn project_fingerprint(py: Python<'_>, project_dir: PathBuf) -> PyResult<Option<String>> {
    let inputs: FingerprintInputs = project_fingerprint_inputs(sqlbuild_version(py)?);
    let fingerprint: Fingerprint = py
        .compiler_detach(|| Ok(scope_fingerprint(&project_dir, &inputs)))
        .map_err(compiler_error)?;
    match fingerprint {
        Fingerprint::Hash(hash) => Ok(Some(hash)),
        Fingerprint::Unavailable => Ok(None),
        Fingerprint::Unserializable => Err(PyTypeError::new_err(
            "a scope-relevant config value is not JSON serializable",
        )),
    }
}
