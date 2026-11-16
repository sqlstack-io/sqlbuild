//! Read `[concurrency_groups]` from `sqlbuild_project.toml`.

use std::collections::BTreeMap;

use crate::project::errors::LoadFailure;
use crate::project::models::ConfigPaths;

/// Each declared group's most runs at once, with `target`'s overrides when one is named.
pub fn load_concurrency_groups(
    paths: &ConfigPaths,
    target: Option<&str>,
) -> Result<BTreeMap<String, u32>, LoadFailure> {
    crate::project::_helpers::documents::concurrency_groups_config(paths, target)
}
