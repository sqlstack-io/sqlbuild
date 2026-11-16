//! Write each `(path, contents)` artifact; returns how many were written and left unchanged.

use crate::compile_outputs::_helpers::artifacts::artifact_error;
use pyo3::prelude::{PyResult, Python};
use sqlbuild_cache::artifacts::errors::ArtifactError;
use sqlbuild_cache::artifacts::main::write_artifacts::write_artifacts;
use sqlbuild_cache::artifacts::models::WrittenArtifacts;
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;
use std::path::PathBuf;

/// Write each `(path, contents)` artifact; returns how many were written and left unchanged.
pub fn write_compiled_artifacts(
    py: Python<'_>,
    files: Vec<(PathBuf, Vec<u8>)>,
    check_existing: bool,
) -> PyResult<(usize, usize)> {
    compiler_guard(|| {
        let outcome: Result<WrittenArtifacts, ArtifactError> =
            py.detach(|| write_artifacts(&files, check_existing));
        let written: WrittenArtifacts = outcome.map_err(|error| artifact_error(py, error))?;
        Ok((written.written, written.unchanged))
    })
}
