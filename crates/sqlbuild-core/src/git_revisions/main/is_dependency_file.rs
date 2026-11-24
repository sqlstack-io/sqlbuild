//! `is_dependency_file`: whether a file name is a Python dependency manifest or lock file.

use crate::git_revisions::_helpers::dependency_files;

/// Whether `name`, such as `uv.lock` or `requirements-dev.txt`, records a project's Python dependencies.
#[must_use]
pub fn is_dependency_file(name: &str) -> bool {
    dependency_files::is_dependency_file(name)
}
