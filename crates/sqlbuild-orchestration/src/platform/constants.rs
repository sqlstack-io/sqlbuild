//! Bounds shared by the platform event reader and webhook dispatcher.

/// Operator-owned delivery endpoint; never inherited by project workers or printed in logs.
pub const PLATFORM_WEBHOOK_VARIABLE: &str = "SQLBUILD_PLATFORM_WEBHOOK_URL";

pub(crate) const PLATFORM_EVENT_PAGE_SIZE: i64 = 100;

/// A run-failure event names at most this many failed nodes and reports the full count beside them.
pub(crate) const RUN_FAILURE_NODE_LIMIT: i64 = 20;

/// Replica heartbeats older than this are pruned; a day's date-scoped invocation is complete by then.
pub(crate) const HEARTBEAT_RETENTION_DAYS: i64 = 2;

/// Platform alerts older than this are pruned; a condition still present then is recorded again.
pub(crate) const PLATFORM_ALERT_RETENTION_DAYS: i64 = 30;

/// Location work past its expiry, and instances unseen for this long, are pruned.
pub(crate) const LOCATION_RETENTION_DAYS: i64 = 7;

/// Sensor evaluations older than this are pruned, except each sensor's latest attempt.
pub(crate) const SENSOR_EVALUATION_RETENTION_DAYS: i64 = 7;

/// Bounded retention deletions per transaction.
pub(crate) const HEARTBEAT_PAGE: usize = 1000;

/// A plan document larger than this is refused by the server and replaced by an error on the location.
pub const PLAN_DOCUMENT_BYTES: usize = 8 * 1024 * 1024;

/// How long a location may hold a plan before another instance may claim it.
pub(crate) const PLAN_LEASE_SECONDS: i64 = 300;

/// A plan nobody claims or finishes by then is reported as expired.
pub(crate) const PLAN_EXPIRY_SECONDS: i64 = 600;

/// Unfinished plans one target accepts across everyone; each person may have one.
pub(crate) const PLAN_QUEUE_LIMIT: i64 = 4;

/// A location instance unseen for longer than this cannot take a plan; locations report every 30 seconds.
pub(crate) const LOCATION_SILENCE_SECONDS: i64 = 90;

/// A SQLite WAL reset waits this long for readers while holding the write lock, then retries later.
pub(crate) const CHECKPOINT_READER_WAIT_MS: u64 = 200;
