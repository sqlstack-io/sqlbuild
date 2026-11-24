//! Where authored source and schema YAML files live.

/// The directory whose direct YAML children declare sources.
pub(crate) const SOURCES_DIRECTORY: &str = sqlbuild_core::constants::SOURCES_PATH;
pub(crate) const MODELS_DIRECTORY: &str = sqlbuild_core::constants::MODELS_PATH;
pub(crate) const SEEDS_DIRECTORY: &str = sqlbuild_core::constants::SEEDS_PATH;
/// Python's `YAML_FILE_SUFFIXES`.
pub(crate) const YAML_FILE_SUFFIXES: [&str; 2] = [".yml", ".yaml"];
/// Python's `SCHEMA_FILE_NAME`.
pub(crate) const SCHEMA_FILE_NAME: &str = "schema.yml";
pub(crate) const SEED_DECLARATION_PATTERN: &str = "*.yml";
pub(crate) const UNSUPPORTED_SEED_DECLARATION_PATTERN: &str = "*.yaml";
