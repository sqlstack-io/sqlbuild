//! Run `body` with one snapshot of `project_dir` shared by every discovery call inside it.

use crate::discovery::_helpers::snapshots::snapshot;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// Run `body` with one snapshot of `project_dir` shared by every discovery call inside it.
#[inline]
pub fn in_snapshot<'py, T>(
    project_dir: &Bound<'py, PyAny>,
    body: impl FnOnce() -> PyResult<T>,
) -> PyResult<T> {
    snapshot::in_snapshot(project_dir, body)
}
