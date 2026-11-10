//! Python's planner selector syntax, error codes and messages the graph reproduces.

pub(crate) const MODEL_RESOURCE: &str = "model";
pub(crate) const SOURCE_RESOURCE: &str = "source";
pub(crate) const SEED_RESOURCE: &str = "seed";
pub(crate) const UDF_RESOURCE: &str = "udf";
pub(crate) const TABLE_FUNCTION_RESOURCE: &str = "table_fn";
pub(crate) const SQL_TEST_RESOURCE: &str = "sql_test";
pub(crate) const EXPANSION_MARKER: char = '+';
pub(crate) const PATH_SEPARATOR: char = '~';
pub(crate) const KIND_SEPARATOR: char = ':';
pub(crate) const FOLDER_SEPARATOR: char = '/';
pub(crate) const INTERSECTION_SEPARATOR: char = ',';
pub(crate) const PATTERN_CHARACTERS: [char; 3] = ['*', '?', '['];
pub(crate) const MODEL_ROOT: &str = "models";
pub(crate) const MODEL_ROOT_PREFIX: &str = "models/";
pub(crate) const SQL_FILE_SUFFIX: &str = ".sql";
pub(crate) const TAG_KIND: &str = "tag";
pub(crate) const PATH_KIND: &str = "path";
pub(crate) const TEST_KIND: &str = "test";
pub(crate) const JOB_KIND: &str = "job";
/// Selector kinds Python parses but `sqb compile` cannot map to a compiled resource.
pub(crate) const UNMAPPED_KINDS: [&str; 4] = ["task", "asset", "loader", "check"];
pub(crate) const SUGGESTION_CUTOFF: f64 = 0.8;
pub(crate) const SUGGESTION_LIMIT: usize = 3;
pub(crate) const EMPTY_SELECTOR_CODE: &str = "S001";
pub(crate) const MISPLACED_MARKER_CODE: &str = "S002";
pub(crate) const PATH_SELECTOR_CODE: &str = "S003";
pub(crate) const MISSING_NAME_CODE: &str = "S004";
pub(crate) const UNKNOWN_KIND_CODE: &str = "S005";
pub(crate) const EMPTY_VALUE_CODE: &str = "S006";
pub(crate) const UNKNOWN_NAME_CODE: &str = "S007";
pub(crate) const UNKNOWN_TAG_CODE: &str = "S008";
pub(crate) const UNKNOWN_PATH_CODE: &str = "S009";
pub(crate) const UNMAPPED_KIND_CODE: &str = "S010";
pub(crate) const PATH_ROOT_CODE: &str = "S012";
pub(crate) const UNIT_TEST_CODE: &str = "S013";
pub(crate) const PLANNER_DEFAULT_CODE: &str = "S000";
pub(crate) const PATH_ROOT_ERROR: &str =
    "path selectors require an explicit root: use 'models/' or 'python/'";
pub(crate) const UNIT_TEST_ONLY_TEST_AND_BUILD: &str =
    "only `sqb test` and `sqb build` accept unit-test selectors";
/// `difflib.SequenceMatcher` treats an element as popular only in sequences this long.
pub(crate) const NAME_KIND: &str = "name";
pub(crate) const DEFAULT_UNIQUE_ID_PREFIX: &str = "model";
