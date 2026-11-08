//! Node kinds the rule tree adds to SQLQuack's expression variants, and kinds it refuses.

/// The kind of a join clause, which SQLQuack keeps as a field of its statement.
pub(crate) const JOIN_KIND: &str = "join";
/// The kind of a common table expression, which SQLQuack keeps as a field of its statement.
pub(crate) const CTE_KIND: &str = "cte";
/// Kinds that hold a statement the parser kept as unparsed text, so no rule can see inside it.
pub(crate) const OPAQUE_KINDS: [&str; 2] = ["command", "raw"];
/// Dialect names read with the shared DuckDB grammar; `generic` matches the fingerprint tokenizer.
pub(crate) const DUCKDB_DIALECTS: [&str; 3] = ["duckdb", "motherduck", "generic"];
/// Dialect names read with the native PostgreSQL parser.
pub(crate) const POSTGRES_DIALECTS: [&str; 2] = ["postgres", "postgresql"];
/// The Snowflake dialect name.
pub(crate) const SNOWFLAKE_DIALECT: &str = "snowflake";
/// The SQLite dialect name.
pub(crate) const SQLITE_DIALECT: &str = "sqlite";
/// Stack for the parse-and-walk worker, so deep SQL reaches the parser's depth limit, not a stack overflow.
pub(crate) const TREE_WORKER_STACK_BYTES: usize = 16 * 1024 * 1024;
/// The prefix of the shared parser's guard codes, such as `E_GUARD_NESTING_DEPTH_EXCEEDED`.
pub(crate) const GUARD_CODE_PREFIX: &str = "E_GUARD_";
