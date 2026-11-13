//! Read `[changes]` from `sqlbuild_project.toml`.

use crate::project::errors::LoadFailure;
use crate::project::models::{ChangesConfig, ConfigPaths};

/// What `--changed` counts and how far a change reaches; each field is `None` when unset.
pub fn load_changes_config(paths: &ConfigPaths) -> Result<ChangesConfig, LoadFailure> {
    crate::project::_helpers::documents::changes_config(paths)
}
