//! Leadership timing is shared by embedded and standalone schedulers.

/// Renew well before another process may take over a stopped scheduler.
pub const RENEW_INTERVAL_SECONDS: u64 = 5;
/// A stopped holder cannot delay failover for more than this database-clock interval.
pub const LEASE_TTL_SECONDS: i64 = 30;
/// Bound a calendar declaration, including due times carried over a republish.
pub const MAX_TICKS_PER_BATCH: usize = 10_000;
/// Covered and skipped ticks older than this are deleted.
pub(crate) const TICK_RETENTION_DAYS: i64 = 30;
pub(crate) const TICK_RETENTION_BATCH: usize = 100;
/// Batches per cleanup pass; a longer backlog resumes next cycle, after the lease renews.
pub(crate) const TICK_RETENTION_BATCHES_PER_PASS: usize = 10;
pub(crate) const TICK_RETENTION_INTERVAL_SECONDS: u64 = 60;
/// A trigger whose stored job or snapshot can't be read is evaluated again after this long, so it never holds the scheduler in a busy loop.
pub(crate) const UNREADABLE_TRIGGER_RETRY_SECONDS: i64 = 60;
/// Why a trigger is skipped while its stored job or snapshot can't be read.
pub(crate) const UNREADABLE_TRIGGER: &str = "this trigger's job was published by an earlier SQLBuild release and can't be read; it runs again after its location publishes again";
