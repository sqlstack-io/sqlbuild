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

/// The reason recorded on a person's plan request; plans carry no reason of their own.
pub(crate) const PLAN_REASON: &str = "plan";

/// A plan only reads warehouse metadata, so it never takes a warehouse a viewer names; `run.warehouse` gates that for runs.
pub(crate) const PLAN_IGNORED_SETTING: &str = "warehouse";

/// Hex digits kept of a worker's invocation-namespaced Recall ID: 128 bits.
pub(crate) const NAMESPACED_ID_HEX: usize = 32;
