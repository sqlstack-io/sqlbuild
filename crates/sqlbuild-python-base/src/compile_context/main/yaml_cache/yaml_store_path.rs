//! Where a delta compile keeps loaded YAML documents.

use crate::compile_context::_helpers::yaml_cache;
use std::path::PathBuf;

/// Where a delta compile keeps loaded YAML documents.
#[inline]
pub fn yaml_store_path(cache_dir: &std::path::Path) -> PathBuf {
    yaml_cache::yaml_store_path(cache_dir)
}
