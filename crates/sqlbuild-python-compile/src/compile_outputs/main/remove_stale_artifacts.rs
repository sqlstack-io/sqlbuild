//! Delete unmanaged compiled files and empty directories; returns how many files were removed.

use crate::compile_outputs::_helpers::artifacts::artifact_error;
use pyo3::prelude::{PyResult, Python};
use sqlbuild_cache::artifacts::main::remove_stale::remove_stale;
use sqlbuild_cache::artifacts::main::remove_stale_unless_unchanged::remove_stale_unless_unchanged;
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;
use std::collections::HashSet;
use std::path::PathBuf;

/// Delete unmanaged compiled files and empty directories; returns how many files were removed.
pub fn remove_stale_artifacts(
    py: Python<'_>,
    compiled_dir: PathBuf,
    managed: Vec<PathBuf>,
    stamp: Option<(PathBuf, i64)>,
) -> PyResult<usize> {
    compiler_guard(|| {
        let managed: HashSet<PathBuf> = managed.into_iter().collect();
        py.detach(|| match &stamp {
            Some((stamp_path, racy_window_ns)) => {
                remove_stale_unless_unchanged(&compiled_dir, &managed, stamp_path, *racy_window_ns)
            }
            None => remove_stale(&compiled_dir, &managed),
        })
        .map_err(|error| artifact_error(py, error))
    })
}
