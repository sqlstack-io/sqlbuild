//! Adapter decisions shared by append-only state history reads.

pub(crate) const SQLSERVER_ADAPTER: &str = "sqlserver";
/// Cursor key of a table without event IDs: always NULL, so it never orders or binds a value.
pub(crate) const MISSING_EVENT_ID_KEY: &str = "NULL";
