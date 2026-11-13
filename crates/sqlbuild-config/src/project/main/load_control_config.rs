//! Read `[control]` from `sqlbuild_project.toml`.

use crate::project::errors::LoadFailure;
use crate::project::models::{ConfigPaths, ControlConfig};

/// The configured server, location and safety sync interval; every field is `None` when unset.
pub fn load_control_config(paths: &ConfigPaths) -> Result<ControlConfig, LoadFailure> {
    crate::project::_helpers::documents::control_config(paths)
}
