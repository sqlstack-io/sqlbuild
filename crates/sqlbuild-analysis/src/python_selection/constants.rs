//! Python node kinds, selector kinds, error codes and messages of Python's node selectors.

pub(crate) const TASK_KIND: &str = "task";
pub(crate) const ASSET_KIND: &str = "asset";
pub(crate) const LOADER_KIND: &str = "loader";
pub(crate) const CHECK_KIND: &str = "check";
pub(crate) const NAME_KIND: &str = "name";
pub(crate) const TAG_KIND: &str = "tag";
pub(crate) const PATH_KIND: &str = "path";
pub(crate) const TEST_KIND: &str = "test";
pub(crate) const JOB_KIND: &str = "job";
pub(crate) const SEED_KIND: &str = "seed";
pub(crate) const SOURCE_KIND: &str = "source";
pub(crate) const MODEL_RESOURCE: &str = "model";
pub(crate) const SOURCE_RESOURCE: &str = "source";
/// Resource types a SQL model may depend on.
pub(crate) const SQL_MODEL_DEPENDENCY_TYPES: [&str; 6] =
    ["model", "source", "seed", "udf", "table_fn", "dbt_ref"];
pub(crate) const EXPANSION_MARKER: char = '+';
pub(crate) const INTERSECTION_SEPARATOR: char = ',';
pub(crate) const PATTERN_CHARACTERS: [char; 3] = ['*', '?', '['];
pub(crate) const SQL_MODEL_PATH_ROOT: &str = "models";
pub(crate) const CURRENT_DIRECTORY: &str = ".";
pub(crate) const ROOT_DIRECTORY: &str = "/";
pub(crate) const PLANNER_DEFAULT_CODE: &str = "S000";
pub(crate) const UNKNOWN_NAME_CODE: &str = "S007";
pub(crate) const UNKNOWN_TAG_CODE: &str = "S008";
pub(crate) const UNKNOWN_PATH_CODE: &str = "S009";
pub(crate) const UNMAPPED_KIND_CODE: &str = "S010";
pub(crate) const PATH_BETWEEN_CODE: &str = "S011";
pub(crate) const PATH_ROOT_CODE: &str = "S012";
pub(crate) const UNIT_TEST_CODE: &str = "S013";
pub(crate) const PATH_ROOT_ERROR: &str =
    "path selectors require an explicit root: use 'models/' or 'python/'";
pub(crate) const UNIT_TEST_ONLY_TEST_AND_BUILD: &str =
    "only `sqb test` and `sqb build` accept unit-test selectors";
/// Codes `sqb check` ignores for an exclude token that matches no Python node.
pub(crate) const IGNORED_EXCLUDE_CODES: [&str; 3] =
    [UNKNOWN_NAME_CODE, UNKNOWN_TAG_CODE, UNKNOWN_PATH_CODE];
