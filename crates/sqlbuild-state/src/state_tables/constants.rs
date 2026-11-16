//! Adapter decisions shared by append-only state history reads.

pub(crate) const SQLSERVER_ADAPTER: &str = "sqlserver";
/// Cursor key of a table without event IDs: always NULL, so it never orders or binds a value.
pub(crate) const MISSING_EVENT_ID_KEY: &str = "NULL";
/// The trailing cursor key of a table whose projected columns already include `event_id`: aliased,
/// so `ORDER BY event_id` names one output column (Snowflake rejects a duplicated output name).
pub(crate) const EVENT_ID_KEY_PROJECTION: &str = ", event_id AS sqb_event_id_key";
/// The rank a newest-versions read filters on; never projected, so it never reaches a record.
pub(crate) const NEWEST_VERSION_COLUMN: &str = "sqb_version";
/// When an observation's first copy was written, so its copies share one rank.
pub(crate) const NEWEST_FIRST_COLUMN: &str = "sqb_first_at";
