//! Publish staged artifacts: `("changed", [])`, `("tree", [])` or `("files", [(source, path)])`.

use crate::compile_outputs::_helpers::artifacts::PublishedRow;
use crate::compile_outputs::_helpers::artifacts::artifact_error;
use crate::compile_outputs::_helpers::artifacts::lossy;
use pyo3::prelude::{PyResult, Python};
use sqlbuild_cache::artifacts::main::publish_staged::publish_staged;
use sqlbuild_cache::artifacts::models::Publication;
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;
use std::collections::HashSet;
use std::path::PathBuf;

/// Publish staged artifacts: `("changed", [])`, `("tree", [])` or `("files", [(source, path)])`.
pub fn publish_staged_artifacts(
    py: Python<'_>,
    staged_dir: PathBuf,
    compiled_dir: PathBuf,
    expected: Vec<PathBuf>,
) -> PyResult<PublishedRow> {
    compiler_guard(|| {
        let expected: HashSet<PathBuf> = expected.into_iter().collect();
        let publication: Publication = py
            .detach(|| publish_staged(&staged_dir, &compiled_dir, &expected))
            .map_err(|error| artifact_error(py, error))?;
        Ok(match publication {
            Publication::StagedChanged => ("changed".to_owned(), Vec::new()),
            Publication::MovedTree => ("tree".to_owned(), Vec::new()),
            Publication::Files(files) => (
                "files".to_owned(),
                files
                    .into_iter()
                    .map(|(source, path)| (lossy(&source), lossy(&path)))
                    .collect(),
            ),
        })
    })
}
