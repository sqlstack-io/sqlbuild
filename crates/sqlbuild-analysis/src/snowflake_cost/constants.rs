//! Snowflake query types, table modifiers and the SQLBuild query-tag identity.

pub(crate) const USE_QUERY_TYPE: &str = "USE";
pub(crate) const DROP_QUERY_TYPE: &str = "DROP";
pub(crate) const CREATE_QUERY_TYPE: &str = "CREATE";
pub(crate) const DYNAMIC_TABLE_MODIFIER: &str = "DYNAMIC";
pub(crate) const CREATE_VARIANT_PREFIX: &str = "create";
pub const QUERY_TAG_APP: &str = "sqlbuild";
pub(crate) const QUERY_TAG_APP_KEY: &str = "app";
pub(crate) const QUERY_TAG_VERSION_KEY: &str = "v";
/// Snowflake's analysis dialect name, which chooses its parser.
pub(crate) const SNOWFLAKE_DIALECT: &str = "snowflake";
