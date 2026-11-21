//! Limits of run output batches, pages and searches, and the names its index stores.

/// The node key of lines a run printed outside any node.
pub const RUN_OUTPUT_NODE: &str = "run";

pub const STDOUT_STREAM: &str = "stdout";

pub const STDERR_STREAM: &str = "stderr";

/// A worker sends a batch once its text reaches this many bytes.
pub const BATCH_TEXT_BYTES: usize = 4 * 1024 * 1024;

/// A worker sends a batch at least this often while output arrives.
pub const BATCH_WINDOW: std::time::Duration = std::time::Duration::from_secs(2);

/// A batch the server accepts carries at most this many text bytes.
pub const MAX_BATCH_TEXT_BYTES: usize = 8 * 1024 * 1024;

/// A batch the server accepts carries at most this many segments.
pub const MAX_BATCH_SEGMENTS: usize = 4096;

/// The most rows one page of output returns.
pub const MAX_PAGE_ROWS: u32 = 2000;

/// Rows one page returns when the reader names no limit.
pub const DEFAULT_PAGE_ROWS: u32 = 500;

/// A folding page reads at most this many lines past its last row to finish a run of similar lines.
pub const COLLAPSE_SCAN_LINES: i64 = 100_000;

/// Four or more lines that are the same but for their numbers fold into one row.
pub const MIN_SIMILAR_LINES: usize = 4;

/// Lines this close to the line a reader jumped to never fold.
pub const AROUND_OPEN_LINES: i64 = 3;

/// A mark keeps at most this many bytes of its line.
pub const MARK_TEXT_BYTES: usize = 300;

/// A page or search reads a view's segments this many at a time.
pub(crate) const SEGMENT_BATCH: i64 = 64;

pub(crate) const ERROR_LEVEL: &str = "error";

pub(crate) const WARNING_LEVEL: &str = "warning";

/// The most chunks one search reads before it hands back where to continue; a view this small is searched whole, with a count.
pub const SEARCH_CHUNKS: usize = 64;

/// Each node's stream keeps at most this many marked lines.
pub const MARKED_LINES: usize = 10_000;

/// The zstd level each chunk is written at.
pub const ZSTD_LEVEL: i32 = 3;

pub(crate) const HEAD_PART: &str = "head";

pub(crate) const TAIL_PART: &str = "tail";

/// The longest `ctx.log` message a worker reports; longer ones end in an ellipsis.
pub const NODE_LOG_MESSAGE_BYTES: usize = 4096;

/// The most `ctx.log` lines a worker reports per node.
pub const NODE_LOGS_PER_NODE: usize = 200;

/// The folder an output store defaults to, beside a SQLite control store.
pub const DEFAULT_OUTPUT_FOLDER: &str = "output";

/// A worker holds at most this many batches waiting to send; past them the printing thread waits.
pub const MAX_QUEUED_BATCHES: usize = 4;

/// How long a printing thread waits for a full queue to drain before it drops the newest batches and counts them.
pub const UPLOAD_STALL: std::time::Duration = std::time::Duration::from_secs(30);

/// A line longer than this is sent in pieces rather than held until its newline.
pub const MAX_PARTIAL_LINE_BYTES: usize = 1024 * 1024;

/// A batch is sent at most this many times while the server is unreachable or busy.
pub const UPLOAD_ATTEMPTS: u32 = 3;

/// How long a finishing worker waits for its last batches to send.
pub const UPLOAD_CLOSE_BUDGET: std::time::Duration = std::time::Duration::from_secs(30);

pub(crate) const FILE_SCHEME: &str = "file://";

pub(crate) const S3_SCHEME: &str = "s3://";

/// Retention removes the output of at most this many runs per pass.
pub(crate) const PRUNE_RUNS: i64 = 100;
