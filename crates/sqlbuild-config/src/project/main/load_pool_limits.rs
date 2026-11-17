//! Read `[pools]` and each target's pool limits from `sqlbuild_project.toml`.

use crate::project::errors::LoadFailure;
use crate::project::models::{ConfigPaths, PoolLimits};

/// Each declared pool's limit, and its limit on every target in `[targets]`.
pub fn load_pool_limits(paths: &ConfigPaths) -> Result<PoolLimits, LoadFailure> {
    crate::project::_helpers::documents::pool_limits_config(paths)
}
