//! Constants of warehouse type parsing.

/// The tree key of a cast.
pub(crate) const CAST_KEY: &str = "cast";
/// The tree key of a cast's target type.
pub(crate) const CAST_TARGET_KEY: &str = "to";
/// The dialect whose type names go through PostgreSQL's own type grammar.
pub(crate) const POSTGRES_DIALECT: &str = "postgres";
/// The dialect whose type names are parsed as a Snowflake cast's target.
pub(crate) const SNOWFLAKE_DIALECT: &str = "snowflake";
/// The dialect whose type names are parsed as a BigQuery cast's target.
pub(crate) const BIGQUERY_DIALECT: &str = "bigquery";
