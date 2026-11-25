//! Nonterminal node states; terminal states come from NodeOutcome.

pub(crate) const NODE_PENDING: &str = "pending";
pub(crate) const NODE_RUNNING: &str = "running";
pub(crate) const NODE_FAILED: &str = "failed";
/// The node states a rerun from failure, an automatic retry and a group treat as unfinished, as a SQL list.
pub(crate) const UNFINISHED_NODE_STATUSES: &str = "'failed','skipped','cancelled','not_run'";

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

/// Days a finished run keeps its worker log by default.
pub(crate) const DEFAULT_RUN_LOG_RETENTION_DAYS: u16 = 14;
/// Finished runs per location target that keep their worker logs by default.
pub(crate) const DEFAULT_RUN_LOG_RETENTION_COUNT: u16 = 200;
/// Runs whose logs one writer transaction deletes.
pub(crate) const RUN_LOG_RETENTION_BATCH: usize = 10;
/// Artifact digests checked for remaining references per statement.
pub(crate) const RUN_LOG_DIGEST_CHUNK: usize = 500;
/// The UUID version of the run IDs the server derives from other IDs, which people may not supply.
pub(crate) const DERIVED_RUN_ID_VERSION: usize = 5;

/// A load runs as a build of its managed sources, so the worker's verified build path enforces its write set.
pub(crate) const LOAD_VERB: &str = "load";

/// The ad-hoc setting that carries `--vars`.
pub(crate) const VARS_SETTING: &str = "vars";

/// Whether group child `r` was retried inside its group: a later attempt of the same member reruns it.
pub(crate) const SUPERSEDED_CHILD: &str = "EXISTS(SELECT 1 FROM sqb_runs s WHERE s.parent_run_id=r.parent_run_id AND s.rerun_of_run_id=r.run_id)";
/// The group run event recorded when a failed member is queued to run again.
pub(crate) const MEMBER_RETRYING_EVENT: &str = "run_member_retrying";
/// The scheduler's launch fact, which names the container a worker started in, if any.
pub(crate) const RUN_LAUNCHED_EVENT: &str = "run_launched";
/// A run's report of what its lifecycle event sinks delivered.
pub(crate) const SINK_DELIVERIES_EVENT: &str = "run_sink_deliveries";
/// The group run event recorded when a retried member succeeds.
pub(crate) const MEMBER_RECOVERED_EVENT: &str = "run_member_recovered";

/// Prepared publications kept per location and target: the current snapshot and the one before it.
pub(crate) const PUBLICATIONS_PER_TARGET: usize = 2;

/// Selected keys per page of a selection evaluation, and the most a request may ask for.
pub(crate) const SELECTION_PAGE_DEFAULT: u32 = 1_000;
pub(crate) const SELECTION_PAGE_MAX: u32 = 10_000;
/// The key prefix of an audit a selection picks, singular or attached.
pub(crate) const AUDIT_KEY_PREFIX: &str = "audit:";

/// A followed run's repeated output reads are audited at most once per person, run and kind in this window, on each server.
pub(crate) const REPEATED_READ_AUDIT: std::time::Duration = std::time::Duration::from_secs(600);

/// The code a node fails with when its provider is missing secrets.
pub(crate) const MISSING_SECRET_ERROR_CODE: &str = "D012";
/// How a worker's message for such a node starts, and the phrase after its variables.
pub(crate) const MISSING_SECRET_PREFIX: &str = "provider ";
pub(crate) const MISSING_SECRET_PHRASE: &str = " not set (missing from ";
/// The fewest characters of a run ID `run:`, `failed:` and `skipped:` take, like a short git hash.
pub(crate) const MIN_RUN_ID_PREFIX: usize = 8;
/// How many runs an ambiguous run ID prefix lists.
pub(crate) const MAX_LISTED_RUN_MATCHES: usize = 5;
/// The most failed audits `failed:` reads from one run.
pub(crate) const PAST_RUN_AUDIT_LIMIT: usize = 10_000;
/// Run node statuses `failed:` selects: what failed, was cancelled or was never reached.
pub(crate) const FAILED_RUN_NODE_STATUSES: [&str; 3] = ["failed", "cancelled", "not_run"];
/// The run node status `skipped:` selects.
pub(crate) const SKIPPED_RUN_NODE_STATUS: &str = "skipped";
/// An audit check that was not run because the target lacks a read: neither run nor failed, so run selectors leave it out.
pub(crate) const NOT_RUN_STATUS: &str = "not_run";
/// The kind of a run node row that records an attached check.
pub(crate) const CHECK_KIND: &str = "check";
