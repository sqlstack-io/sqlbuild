//! Read which `[settings]` keys `sqlbuild_project.toml` sets itself, rather than leaving to defaults.

use crate::project::errors::LoadFailure;
use crate::project::models::ConfigPaths;

/// The `[settings]` keys the project file sets, under their current names.
pub fn load_project_setting_keys(paths: &ConfigPaths) -> Result<Vec<String>, LoadFailure> {
    crate::project::_helpers::documents::project_setting_keys(paths)
}
