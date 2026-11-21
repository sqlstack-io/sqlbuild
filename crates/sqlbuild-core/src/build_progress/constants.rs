//! Column widths, limits and status values of build progress output.

pub(crate) const TYPE_WIDTH: usize = 10;
pub(crate) const MIN_NAME_WIDTH: usize = 20;
pub(crate) const NAME_PADDING: usize = 2;
pub(crate) const SUB_INDENT: usize = 2;
pub(crate) const TREE_CHILD_NAME_INDENT: usize = SUB_INDENT + 4;
pub(crate) const HOOK_PHASE_WIDTH: usize = 10;
pub(crate) const HOOK_TYPE_WIDTH: usize = 8;
pub(crate) const HOOK_MIN_LABEL_WIDTH: usize = 24;
pub(crate) const HOOK_MAX_LABEL_WIDTH: usize = 56;
pub(crate) const SKIPPED_ROLLUP_CAP: usize = 20;
pub(crate) const ROW_COUNT_BILLION: i64 = 1_000_000_000;
pub(crate) const ROW_COUNT_MILLION: i64 = 1_000_000;
pub(crate) const ROW_COUNT_THOUSAND: i64 = 1_000;
pub(crate) const ROW_COUNT_ABBREVIATION_THRESHOLD: i64 = 10_000;
pub(crate) const SECONDS_PER_MINUTE: f64 = 60.0;
pub(crate) const SCHEDULER_DIAGNOSTIC_LIMIT: usize = 50;
pub(crate) const QUERY_DIAGNOSTIC_LIMIT: usize = 25;
pub(crate) const SPINNER_FRAMES: [&str; 10] = [
    "\u{280b}", "\u{2819}", "\u{2839}", "\u{2838}", "\u{283c}", "\u{2834}", "\u{2826}", "\u{2827}",
    "\u{2807}", "\u{280f}",
];
pub(crate) const BATCH_INDENT: &str = "         ";
pub(crate) const LOG_LABEL: &str = "    log  ";
pub(crate) const LOG_CONTINUATION: &str = "         ";
pub(crate) const PYTHON_NODE_NAME_WIDTH: usize = 50;
pub(crate) const DELTA_SUFFIX: &str = "__delta";

pub(crate) const SUCCESS: &str = "success";
pub(crate) const FAILED: &str = "failed";
pub(crate) const SKIPPED: &str = "skipped";
pub(crate) const PASS: &str = "pass";
pub(crate) const WARN: &str = "warn";
pub(crate) const ERROR: &str = "error";
pub(crate) const INSUFFICIENT: &str = "insufficient";
pub(crate) const DELTA_AND_FINAL: &str = "delta_and_final";
pub(crate) const TIMEOUT_OUTCOME: &str = "timeout";
pub(crate) const TIMEOUT_CODE: &str = "C417";

/// Seconds before each heartbeat of captured build output; the last repeats.
pub(crate) const HEARTBEAT_INTERVALS: [u64; 4] = [30, 60, 120, 300];

/// How many running nodes a heartbeat names before `+N more`.
pub(crate) const HEARTBEAT_NAMES: usize = 3;
