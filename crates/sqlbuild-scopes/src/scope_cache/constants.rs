//! Cache schema and fingerprint versions, keys and the scope-relevant source facts.

use sqlbuild_core::constants::PROJECT_CONFIG_FILENAME;

pub(crate) const CACHE_SCHEMA_VERSION: i64 = 3;
pub(crate) const FINGERPRINT_ALGORITHM_VERSION: i64 = 2;
/// The largest cache file read; bigger files are misses.
pub(crate) const CACHE_MAX_BYTES: u64 = 16 * 1024 * 1024;
pub(crate) const RESOURCE_IDENTITY: &str = "resource";
pub(crate) const DECLARATION_IDENTITY: &str = "declaration";
/// Authored suffixes, compared lowercased, whose contents feed the fingerprint.
pub(crate) const SOURCE_SUFFIXES: [&str; 4] = [".sql", ".yml", ".yaml", ".py"];
pub(crate) const MACRO_SUFFIX: &str = ".py";
pub(crate) const RELATIONSHIP_ROOTS: [(&str, &str); 2] =
    [("tests", "unit"), ("tests", "scenarios")];
pub(crate) const LOCAL_CONFIG_KEYS: [&str; 5] =
    ["adapter", "settings", "target", "targets", "vars"];
pub(crate) const PROJECT_CONFIG_KEYS: [&str; 9] = [
    "adapter",
    "constants",
    "default_target",
    "defaults",
    "path_defaults",
    "scopes",
    "settings",
    "targets",
    "vars",
];
pub(crate) const LOCAL_CONFIG_FILENAME: &str = "sqlbuild_local.toml";
pub(crate) const TARGETS_KEY: &str = "targets";
pub(crate) const TARGET_VARS_KEY: &str = "vars";
pub(crate) const TOML_SUFFIX: &str = ".toml";
pub(crate) const COMPLETENESS_KEYS: [&str; 6] = [
    "discovery",
    "static_visibility",
    "runtime_usage",
    "relationships",
    "placement",
    "promotion_impact",
];
/// Config files the project fingerprint hashes when present.
pub(crate) const FINGERPRINT_CONFIG_FILES: [&str; 4] = [
    PROJECT_CONFIG_FILENAME,
    "sqlbuild_local.toml",
    ".sqlbuildignore",
    ".gitignore",
];
/// Project-wide named declaration folders, sorted.
pub(crate) const NAMED_DECLARATION_DIRECTORIES: [&str; 3] = ["audits", "hooks", "schemas"];
/// Source roots outside the declaration layout whose files feed the fingerprint.
pub(crate) const EXTRA_SOURCE_ROOTS: [&str; 2] = ["python", "sql/seeds"];
