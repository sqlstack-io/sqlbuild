//! Run one command with jobs selectable from `project_dir`, restoring the enclosing scope after.

use crate::compiled_project::_helpers::job_catalog;
use std::path::PathBuf;

/// Run one command with jobs selectable from `project_dir`, restoring the enclosing scope after.
#[inline]
pub fn with_job_scope<T>(
    (project_dir, selected_target): (PathBuf, Option<String>),
    run: impl FnOnce() -> T,
) -> T {
    job_catalog::with_job_scope((project_dir, selected_target), run)
}
