//! Public constants of sql test glue.

pub(crate) const UDF: &str = "udf";

pub(crate) const TABLE_FN: &str = "table_fn";

pub(crate) const ERROR_SEVERITY: &str = "error";

pub(crate) const FIXTURE_PREFIXES: [(&str, &str); 4] = [
    ("model", "__ref__"),
    ("source", "__source__"),
    ("seed", "__seed__"),
    ("sql_test", "__expected__"),
];

pub(crate) const CURSOR_ALGEBRA_EXCEPTIONS: &str = "sqlbuild.cursor_algebra.exceptions";

pub(crate) const INTEGER_CURSOR_TYPE: &str = "integer";

pub(crate) const TIMESTAMP_CURSOR_TYPE: &str = "timestamp";

pub const DEFAULT_TIMESTAMP_START: &str = "1900-01-01 00:00:00";

pub const DEFAULT_TIMESTAMP_END: &str = "2999-12-31 00:00:00";

pub const DEFAULT_INTEGER_START: &str = "-1000000000000000";

pub const DEFAULT_INTEGER_END: &str = "1000000000000000";

pub(crate) const CURSOR_START_KEY: &str = "cursor_start";

pub(crate) const CURSOR_END_KEY: &str = "cursor_end";

pub(crate) const SENTINEL: &str = "__SQLBUILD_CALL_SUFFIX__";

/// The diagnostic code of a SQL test that references a helper it may not read.
pub const SQL_TEST_HELPER_REFERENCE_CODE: &str = "P013";

/// The scan store namespace of expanded SQL-test CTEs.
pub const SQL_TEST_CTE_SCAN_ALGORITHM: &str = "expanded-sql-test-ctes-v1";

/// The scan store namespace of the models SQL tests expect.
pub const SQL_TEST_EXPECTED_MODELS_SCAN_ALGORITHM: &str = "sql-test-expected-models-v1";
