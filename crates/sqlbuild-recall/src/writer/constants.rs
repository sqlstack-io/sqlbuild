//! Queue bounds, batching and timing of the Recall writer.

pub const QUEUE_RECORD_CAPACITY: usize = 10_000;
pub const QUEUE_BYTE_CAPACITY: usize = 32 * 1024 * 1024;
/// A batch is sent before its window once this many bytes are ready, so a long window never fills the queue.
pub const EARLY_BATCH_BYTES: usize = QUEUE_BYTE_CAPACITY / 4;
pub const BATCH_ROWS: usize = 500;
pub const BATCH_WINDOW_MS: u64 = 250;
/// Contiguous writes to one stream coalesce into one chunk for at most this long.
pub const OUTPUT_COALESCE_MS: u64 = 250;
pub const DEFAULT_HEARTBEAT_INTERVAL_MS: u64 = 30_000;
/// An invocation is abandoned once its last heartbeat is this many intervals old.
pub const HEARTBEAT_STALE_INTERVALS: u32 = 3;
/// Longest single backoff between retries of a busy write.
pub const MAX_RETRY_BACKOFF_MS: u64 = 500;
/// A batch that stays busy this long while the command runs is counted as failed.
pub const RUNNING_RETRY_LIMIT_MS: u64 = 30_000;
/// Pause between setup attempts while another writer holds the store.
pub const SETUP_RETRY_MS: u64 = 50;
/// Longest wait between wall-clock checks, so a resumed process beats promptly.
pub const CLOCK_CHECK_MS: u64 = 1_000;
/// Least time the final delivery rows get; rows landing after close then leave the CLI under-reporting, not the store.
pub const FINAL_ROWS_BUDGET_MS: u64 = 500;
/// Maintenance is skipped when less than this remains of the shutdown budget.
pub const MINIMUM_MAINTENANCE_MS: u64 = 200;
/// SQL longer than this is counted as dropped without hashing or copying it.
pub const MAX_ARTIFACT_BYTES: usize = 4 * 1024 * 1024;
/// What a JSON scalar counts toward the queue's byte capacity.
pub const JSON_SCALAR_BYTES: usize = 8;
/// What a lifecycle event awaiting its replacement sink counts toward the queue's byte capacity.
pub const PENDING_EVENT_BYTES: usize = 1024;
