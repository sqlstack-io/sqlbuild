//! Recall binding names, environment variables and error codes.

/// Seconds between heartbeats; tests shorten it to observe abandoned invocations quickly.
pub(crate) const HEARTBEAT_ENVIRONMENT: &str = "SQLBUILD_RECALL_HEARTBEAT_SECONDS";
/// The commit a deployment without `.git` records as `git_revision`.
pub(crate) const GIT_REVISION_ENVIRONMENT: &str = "SQLBUILD_GIT_REVISION";
/// Whether that deployment's tree had uncommitted changes, recorded as `git_dirty`.
pub(crate) const GIT_DIRTY_ENVIRONMENT: &str = "SQLBUILD_GIT_DIRTY";
/// Marker files under `target/` limiting store notices to one per interval.
pub(crate) const DDL_NOTICE_PREFIX: &str = ".recall-notice-";
pub(crate) const NOTICE_INTERVAL_SECONDS: u64 = 24 * 60 * 60;
/// Hex digits of the hostname digest recorded as the host token.
pub(crate) const HOST_TOKEN_LENGTH: usize = 16;
/// The shortest heartbeat interval the test environment variable may request.
pub(crate) const MINIMUM_HEARTBEAT_SECONDS: f64 = 0.01;
/// What a Recall closing notice says is the only thing affected.
pub(crate) const RECALL_HISTORY: &str = "Recall history";
/// The database the ClickHouse client takes as none, so the server uses the user's default: a store whose database is missing connects with it to create or report the database.
pub(crate) const SERVER_DEFAULT_DATABASE: &str = "__default__";
/// The compute-log stream index of stdout; every other output index is stderr.
pub(crate) const STDOUT_STREAM_INDEX: usize = 0;
pub(crate) const RECALL_DISABLED_CODE: &str = "C970";
pub(crate) const RECALL_STORE_CODE: &str = "C971";
pub(crate) const RECALL_NOT_FOUND_CODE: &str = "C972";
pub(crate) const RECALL_AMBIGUOUS_CODE: &str = "C973";
pub(crate) const RECALL_QUERY_CODE: &str = "C974";
pub(crate) const RECALL_SETTINGS_CODE: &str = "C975";
pub(crate) const WAREHOUSE_BACKEND: &str = "warehouse";
/// SQLBuild's state relations in the warehouse; a prefix comparison needs no `LIKE` escape, whose syntax differs by dialect.
pub(crate) const WAREHOUSE_STATE_RELATIONS_SQL: &str = "SELECT table_schema, table_name, table_type \
     FROM information_schema.tables WHERE SUBSTRING(table_name, 1, 10) = '_sqlbuild_' \
     ORDER BY table_schema, table_name";
/// Default cell width in text formats, in bytes.
pub(crate) const DEFAULT_MAX_CELL_BYTES: usize = 4096;
pub(crate) const JSON_FORMAT: &str = "json";
/// How text output shows SQL NULL.
pub(crate) const NULL_TEXT: &str = "NULL";
/// Join keys `sqb recall schema` points out.
pub(crate) const JOIN_KEYS: [&str; 5] = [
    "invocation_id",
    "statement_id",
    "sql_digest",
    "run_id",
    "resource_id",
];
/// Rows `sqb recall query` returns without `--limit` or `--no-limit`.
pub(crate) const DEFAULT_QUERY_LIMIT: i64 = 100;
/// The invocation selector naming the most recently started invocation.
pub(crate) const LAST_INVOCATION: &str = "last";
/// The keyword arguments the `sqb recall` dispatch route passes.
pub(crate) const RECALL_OPTIONS: [&str; 14] = [
    "project_dir",
    "selected_target",
    "recall_command",
    "sql",
    "backend",
    "output_format",
    "limit",
    "no_limit",
    "max_cell_bytes",
    "relation",
    "invocation",
    "stream",
    "tail",
    "ddl",
];
/// Required text envelope fields of a known lifecycle event.
pub(crate) const EVENT_TEXT_FIELDS: [&str; 5] = [
    "event_id",
    "event_type",
    "producer",
    "producer_version",
    "invocation_id",
];
/// Optional correlation fields of a known lifecycle event.
pub(crate) const EVENT_OPTIONAL_TEXT_FIELDS: [&str; 5] = [
    "run_id",
    "resource_id",
    "resource_attempt_id",
    "operation_id",
    "statement_id",
];
/// The first lifecycle schema version carrying invocation sequence and external context.
pub(crate) const INVOCATION_METADATA_SCHEMA_VERSION: i64 = 2;
/// `datetime` attributes of an ISO-8601 timestamp, most significant first.
pub(crate) const TIMESTAMP_PARTS: [&str; 7] = [
    "year",
    "month",
    "day",
    "hour",
    "minute",
    "second",
    "microsecond",
];
/// `str(datetime.UTC)`.
pub(crate) const UTC_ZONE_NAME: &str = "UTC";
/// How often closing checks for an interrupt while delivery finishes.
pub(crate) const CLOSE_POLL_MS: u64 = 50;
/// Longest wait at startup for another process moving a corrupt store aside; the writer thread retries within the shutdown budget.
pub(crate) const QUARANTINE_START_WAIT_MS: u64 = 200;
/// The ClickHouse error name of a query naming a relation that does not exist yet.
pub(crate) const UNKNOWN_TABLE_ERROR: &str = "UNKNOWN_TABLE";
/// `sqb recall query` runs read-only: the setting, and the value that still allows query settings. Not sent when the session is already read-only.
pub(crate) const READONLY_SETTING: &str = "readonly";
pub(crate) const READ_ONLY_QUERIES: i64 = 2;
/// Seconds in a day, for the artifact refresh cutoff.
pub(crate) const SECONDS_PER_DAY: u64 = 24 * 60 * 60;
/// The parameter a lifecycle-event sink takes its event in.
pub(crate) const EVENT_PARAMETER: &str = "event";
/// The parameter a command-output sink takes its record in.
pub(crate) const RECORD_PARAMETER: &str = "record";
/// How Recall timestamps mark UTC, and the offset `datetime.fromisoformat` reads.
pub(crate) const UTC_SUFFIX: &str = "Z";
pub(crate) const UTC_OFFSET: &str = "+00:00";
/// The command whose flags name its Recall `command_mode`.
pub(crate) const DEBUG_COMMAND: &str = "debug";
/// The namespace attribute of `sqb debug --no-connection`.
pub(crate) const NO_CONNECTION_ATTRIBUTE: &str = "no_connection";
/// The statement terminal events, at which the sink settles a statement's SQL.
pub(crate) const INVOCATION_STARTED: &str = "invocation_started";
pub(crate) const INVOCATION_COMPLETED: &str = "invocation_completed";
pub(crate) const INVOCATION_FAILED: &str = "invocation_failed";
pub(crate) const STATEMENT_COMPLETED: &str = "statement_completed";
pub(crate) const STATEMENT_FAILED: &str = "statement_failed";
