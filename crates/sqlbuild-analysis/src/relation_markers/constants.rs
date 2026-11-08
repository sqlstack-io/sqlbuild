//! The relation marker call pattern and the names marker calls stand as.

/// The prefix of the name a marker call stands as while parsed; the call's index follows it.
pub const MARKER_NAME_PREFIX: &str = "__sqb_";
/// Any relation marker call: `__name("x")`, or `__name("a", "b")` as `__dbt_ref` writes it.
pub(crate) const RELATION_MARKER_PATTERN: &str =
    r#"(?i)\b(__[a-z_]+)\(\s*"([^"]+)"\s*(?:,\s*"([^"]+)"\s*)?\)"#;
/// The marker whose two names read as one, joined by `__`.
pub(crate) const DBT_REF_FUNCTION: &str = "__dbt_ref";
/// The marker of a table function call, which Snowflake reads only inside `TABLE( )`.
pub(crate) const TABLE_FUNCTION_MARKER: &str = "__table_fn";
