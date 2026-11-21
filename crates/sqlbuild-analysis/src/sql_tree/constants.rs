//! Node kinds the rule tree adds to SQLQuack's expression variants, and kinds it refuses.

/// The kind of a join clause, which SQLQuack keeps as a field of its statement.
pub(crate) const JOIN_KIND: &str = "join";
/// The kind of a common table expression, which SQLQuack keeps as a field of its statement.
pub(crate) const CTE_KIND: &str = "cte";
/// The kind owning a query's select list.
pub(crate) const PROJECTION_KIND: &str = "projection";
/// The kind owning a query's `FROM` sources; explicit joins stay `join` nodes beside it.
pub(crate) const FROM_KIND: &str = "from";
/// The kind owning a `WHERE` predicate.
pub(crate) const WHERE_KIND: &str = "where";
/// The kind owning `GROUP BY` keys.
pub(crate) const GROUP_BY_KIND: &str = "group_by";
/// The kind owning a `HAVING` predicate.
pub(crate) const HAVING_KIND: &str = "having";
/// The kind owning a `QUALIFY` predicate.
pub(crate) const QUALIFY_KIND: &str = "qualify";
/// The kind owning a statement's `ORDER BY` keys.
pub(crate) const ORDER_BY_KIND: &str = "order_by";
/// The kind owning a `LIMIT` count.
pub(crate) const LIMIT_KIND: &str = "limit";
/// The kind owning a window's `PARTITION BY` keys.
pub(crate) const PARTITION_BY_KIND: &str = "partition_by";
/// The kind owning a window's `ORDER BY` keys.
pub(crate) const WINDOW_ORDER_BY_KIND: &str = "window_order_by";
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
