//! Nonterminal node states; terminal states come from NodeOutcome.

pub(crate) const NODE_PENDING: &str = "pending";
pub(crate) const NODE_RUNNING: &str = "running";

pub(crate) const HEARTBEAT_TIMEOUT_SECONDS: i64 = 45;
pub(crate) const MONITOR_BATCH_SIZE: usize = 100;

pub(crate) const OUTPUT_BATCH_CHUNKS: usize = 256;
pub(crate) const OUTPUT_BATCH_BYTES: usize = 1_048_576;
pub(crate) const RECALL_BATCH_ROWS: usize = 500;
pub(crate) const RECALL_INVOCATION_ID_BYTES: usize = 256;
