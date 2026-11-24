//! `named_env_files`: every env file the project's configuration names.

use std::path::{Path, PathBuf};

use crate::env_file::_helpers::choice::named_texts;
use crate::project::main::load_project_config::load_project_config;
use crate::project::main::project_config_paths::project_config_paths;

/// The project's and each target's `env_file`, as written; none without a valid project file.
#[must_use]
pub fn named_env_files(project_dir: &Path) -> Vec<PathBuf> {
    load_project_config(&project_config_paths(project_dir))
        .map(|project| {
            named_texts(&project)
                .into_iter()
                .map(PathBuf::from)
                .collect()
        })
        .unwrap_or_default()
}
