//! In a delta reading only some sources, drop every other loaded source YAML entry.

use crate::compile_context::_helpers::yaml_cache;

/// In a delta reading only some sources, drop every other loaded source YAML entry.
#[inline]
pub fn retain_read_sources(
    outcomes: Vec<sqlbuild_discovery::yaml_files::models::YamlFileOutcome>,
) -> Vec<sqlbuild_discovery::yaml_files::models::YamlFileOutcome> {
    yaml_cache::retain_read_sources(outcomes)
}
