//! Scenario command labels, widths, defaults and error codes.

/// The environment variable that sets the scenario run namespace.
pub const SCENARIO_NAMESPACE_ENV: &str = "SQLBUILD_SCENARIO_NAMESPACE";
/// The longest accepted scenario run namespace.
pub(crate) const MAX_NAMESPACE_LENGTH: usize = 128;
pub(crate) const UNSET_NAMESPACE_LABEL: &str = "(unset)";
pub(crate) const UNSET_NAMESPACE_SOURCE: &str = "unset";
pub(crate) const CLI_SOURCE: &str = "cli";
pub(crate) const ENV_SOURCE: &str = "env";
pub(crate) const LOCAL_CONFIG_SOURCE: &str = "local config";
pub(crate) const PROJECT_CONFIG_SOURCE: &str = "project config";

pub(crate) const SCENARIO_NAME_WIDTH: usize = 64;
pub(crate) const RETAINED_KIND_WIDTH: usize = 6;
pub(crate) const CAPTURE_RELATION_KIND_WIDTH: usize = 8;
pub(crate) const CAPTURE_RELATION_NAME_WIDTH: usize =
    SCENARIO_NAME_WIDTH - 4 - CAPTURE_RELATION_KIND_WIDTH - 1;
pub(crate) const BINARY_UNIT_SIZE: f64 = 1024.0;

pub(crate) const PASS_STATUS: &str = "PASS";
pub(crate) const FAIL_STATUS: &str = "FAIL";

pub(crate) const DEFAULT_MAX_SNAPSHOT_ROWS_PER_RELATION: i64 = 10_000;
pub(crate) const DEFAULT_MAX_SNAPSHOT_TOTAL_ROWS: i64 = 50_000;
pub(crate) const DEFAULT_MAX_SNAPSHOT_BYTES_PER_RELATION: i64 = 5_000_000;
pub(crate) const DEFAULT_MAX_SNAPSHOT_TOTAL_BYTES: i64 = 25_000_000;

pub(crate) const NONE_DISCOVERED_CODE: &str = "C451";
pub(crate) const LOCAL_RETAIN_UNSUPPORTED_CODE: &str = "C452";
pub(crate) const UNKNOWN_SELECTOR_CODE: &str = "C453";
pub(crate) const LOCAL_SNAPSHOT_FLAG_REQUIRED_CODE: &str = "C454";
pub(crate) const SQL_VALIDATION_REQUIRED_CODE: &str = "C455";
pub(crate) const CAPTURE_DIALECT_REQUIRED_CODE: &str = "C456";
pub(crate) const UNSUPPORTED_GRAPH_SELECTOR_CODE: &str = "C457";
pub(crate) const INVALID_NAMESPACE_CODE: &str = "C458";

pub(crate) const GRAPH_SELECTOR_EXPANSION_MARKER: char = '+';
pub(crate) const GRAPH_SELECTOR_PATH_MARKER: char = '~';
pub(crate) const SQL_EXTENSION: &str = "sql";
pub(crate) const SCENARIO_ROOT: [&str; 2] = ["tests", "scenarios"];

pub(crate) const MODELS_DIR: &str = crate::constants::MODELS_PATH;
pub(crate) const FUNCTIONS_DIR: &str = crate::constants::FUNCTIONS_PATH;
pub(crate) const FIXTURES_DIR: &str = "fixtures";
pub(crate) const SEEDS_DIR: &str = crate::constants::SEEDS_PATH;
pub(crate) const EXPECTATIONS_DIR: &str = "expectations";
pub(crate) const CLEANUP_DIR: &str = "cleanup";

pub(crate) const SECONDS_PER_DAY: i64 = 86_400;
pub(crate) const DAYS_PER_ERA: i64 = 146_097;
pub(crate) const EPOCH_DAY_OFFSET: i64 = 719_468;
pub(crate) const YEARS_PER_ERA: i64 = 400;
pub(crate) const FEBRUARY: i64 = 2;
/// The March-based month index of January.
pub(crate) const MARCH_BASED_JANUARY: i64 = 10;
