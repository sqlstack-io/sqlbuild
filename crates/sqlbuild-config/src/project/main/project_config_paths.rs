//! The configuration files a project directory may hold.

use std::path::Path;

use sqlbuild_core::constants::PROJECT_CONFIG_FILENAME;

use crate::project::constants::{
    LEGACY_LOCAL_CONFIG_FILENAME, LEGACY_PROJECT_CONFIG_FILENAME, LOCAL_CONFIG_FILENAME,
};
use crate::project::models::ConfigPaths;

/// The four configuration files of `project_dir`.
pub fn project_config_paths(project_dir: &Path) -> ConfigPaths {
    ConfigPaths {
        project: project_dir.join(PROJECT_CONFIG_FILENAME),
        legacy_project: project_dir.join(LEGACY_PROJECT_CONFIG_FILENAME),
        local: project_dir.join(LOCAL_CONFIG_FILENAME),
        legacy_local: project_dir.join(LEGACY_LOCAL_CONFIG_FILENAME),
    }
}
