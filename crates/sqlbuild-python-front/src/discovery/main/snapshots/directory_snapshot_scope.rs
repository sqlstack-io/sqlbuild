//! Share one directory snapshot across every discovery call for `project_dir` inside the scope.

use crate::discovery::models::NativeDirectorySnapshotScope;
use pyo3::prelude::{Bound, PyAny};
use pyo3::pyfunction;

/// Share one directory snapshot across every discovery call for `project_dir` inside the scope.
#[pyfunction]
#[pyo3(signature = (*, project_dir))]
pub fn directory_snapshot_scope(project_dir: Bound<'_, PyAny>) -> NativeDirectorySnapshotScope {
    NativeDirectorySnapshotScope {
        project_dir: project_dir.unbind(),
        token: None,
    }
}
