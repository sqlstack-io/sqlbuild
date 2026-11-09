//! Nonterminal node states; terminal states come from NodeOutcome.

pub(crate) const NODE_PENDING: &str = "pending";
pub(crate) const NODE_RUNNING: &str = "running";

pub(crate) const HEARTBEAT_TIMEOUT_SECONDS: i64 = 45;
pub(crate) const MONITOR_BATCH_SIZE: usize = 100;

pub(crate) const OUTPUT_BATCH_CHUNKS: usize = 256;
pub(crate) const OUTPUT_BATCH_BYTES: usize = 1_048_576;
pub(crate) const RECALL_BATCH_ROWS: usize = 500;
pub(crate) const RECALL_INVOCATION_ID_BYTES: usize = 256;

/// Manual requests derive their audit event ID from the run ID so retries find the original.
pub(crate) const MANUAL_JOB_AUDIT_TAG: &[u8] = b"manual-job-request";
pub(crate) const RERUN_AUDIT_TAG: &[u8] = b"rerun-request";

/// Ad-hoc requests derive their audit event ID from the run ID so retries find the original.
pub(crate) const ADHOC_AUDIT_TAG: &[u8] = b"adhoc-run-request";

/// The build setting a full-refresh launch adds to the frozen request and each group child.
pub(crate) const FULL_REFRESH_SETTING: &str = "full_refresh";
