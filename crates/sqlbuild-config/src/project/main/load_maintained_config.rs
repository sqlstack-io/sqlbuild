//! Read `[maintained]` from `sqlbuild_project.toml`.

use crate::project::errors::LoadFailure;
use crate::project::models::{ConfigPaths, MaintainedConfig};

/// The maintained-model pause settings, with their defaults when `[maintained]` sets none.
pub fn load_maintained_config(paths: &ConfigPaths) -> Result<MaintainedConfig, LoadFailure> {
    crate::project::_helpers::documents::maintained_config(paths)
}
