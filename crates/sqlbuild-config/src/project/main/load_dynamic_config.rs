//! Read `[dynamic]` from `sqlbuild_project.toml`.

use crate::project::errors::LoadFailure;
use crate::project::models::{ConfigPaths, DynamicConfig};

/// The dynamic-model pause settings, with their defaults when `[dynamic]` sets none.
pub fn load_dynamic_config(paths: &ConfigPaths) -> Result<DynamicConfig, LoadFailure> {
    crate::project::_helpers::documents::dynamic_config(paths)
}
