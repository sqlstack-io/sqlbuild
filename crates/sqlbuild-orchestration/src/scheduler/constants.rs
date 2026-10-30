//! Leadership timing is shared by embedded and standalone schedulers.

/// Renew well before another process may take over a stopped scheduler.
pub const RENEW_INTERVAL_SECONDS: u64 = 5;
/// A stopped holder cannot delay failover for more than this database-clock interval.
pub const LEASE_TTL_SECONDS: i64 = 30;
/// Bound each decision batch, including covered ticks, before opening a write transaction.
pub const MAX_TICKS_PER_BATCH: usize = 10_000;
