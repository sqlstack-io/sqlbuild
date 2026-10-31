//! Nonterminal node states; terminal states come from NodeOutcome.

pub(crate) const NODE_PENDING: &str = "pending";
pub(crate) const NODE_RUNNING: &str = "running";

pub(crate) const HEARTBEAT_TIMEOUT_SECONDS: i64 = 45;
pub(crate) const MONITOR_BATCH_SIZE: usize = 100;
