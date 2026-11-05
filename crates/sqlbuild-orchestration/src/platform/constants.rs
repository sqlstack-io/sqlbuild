//! Bounds shared by the platform event reader and webhook dispatcher.

/// Operator-owned delivery endpoint; never inherited by project workers or printed in logs.
pub const PLATFORM_WEBHOOK_VARIABLE: &str = "SQLBUILD_PLATFORM_WEBHOOK_URL";

pub(crate) const PLATFORM_EVENT_PAGE_SIZE: i64 = 100;

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
