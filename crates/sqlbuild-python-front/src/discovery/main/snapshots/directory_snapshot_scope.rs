//! Share one directory snapshot across every discovery call for `project_dir` inside the scope.

use crate::discovery::_helpers::snapshots::snapshot;
use crate::discovery::models::NativeDirectorySnapshotScope;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;

/// Share one directory snapshot across every discovery call for `project_dir` inside the scope.
#[inline]
pub fn directory_snapshot_scope(project_dir: Bound<'_, PyAny>) -> NativeDirectorySnapshotScope {
    snapshot::directory_snapshot_scope(project_dir)
}
