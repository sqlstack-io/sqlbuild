//! The env file a project directory and target choose.

use std::path::Path;

use crate::env_file::_helpers::choice::{choice, configured_source};
use crate::env_file::models::EnvFileChoice;
use crate::project::main::load_project_config::load_project_config;
use crate::project::main::project_config_paths::project_config_paths;

/// The chosen env file, `None` without a valid `sqlbuild_project.toml`, which the command itself reports.
#[must_use]
pub fn env_file_choice(project_dir: &Path, target: Option<&str>) -> Option<EnvFileChoice> {
    let paths = project_config_paths(project_dir);
    if !paths.project.is_file() {
        return None;
    }
    let project = load_project_config(&paths).ok()?;
    Some(choice(configured_source(&project, (project_dir, target))))
}
