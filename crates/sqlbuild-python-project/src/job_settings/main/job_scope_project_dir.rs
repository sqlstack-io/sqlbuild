//! The project directory of the running command's job scope.

use crate::job_settings::_helpers::job_catalog;
use std::path::PathBuf;

/// The project directory of the running command's job scope, if it has one.
#[inline]
pub fn job_scope_project_dir() -> Option<PathBuf> {
    job_catalog::job_scope_project_dir()
}
