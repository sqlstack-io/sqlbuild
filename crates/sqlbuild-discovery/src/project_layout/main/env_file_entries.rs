//! `env_file_entries`: the project's top-level entries that hold the env files it names.

use std::path::{Path, PathBuf};

use sqlbuild_config::env_file::main::named_env_files::named_env_files;

use crate::project_layout::_helpers::written::entries_holding;

/// The top-level entries of `root` holding an env file its `env_file` settings name.
#[must_use]
pub fn env_file_entries(root: &Path) -> Vec<String> {
    let named: Vec<PathBuf> = named_env_files(root);
    entries_holding(root, &named)
}
