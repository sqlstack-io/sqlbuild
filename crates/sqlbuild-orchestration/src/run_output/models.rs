//! Worker output batches, the store that keeps them, and the index, pages and searches people read.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// Where a server keeps run output: a local folder or an S3 prefix, and how much of a run it keeps.
#[derive(Clone, Debug)]
pub struct OutputStore {
    pub(crate) objects: Arc<dyn object_store::ObjectStore>,
    pub(crate) prefix: object_store::path::Path,
    pub(crate) location: String,
    pub(crate) max_bytes_per_run: Option<u64>,
}

impl OutputStore {
    /// The store's location as configured, such as `file:///var/lib/sqlbuild/output`.
    #[must_use]
    pub fn location(&self) -> &str {
        &self.location
    }
}

/// `server.toml`'s `[output]`: the store's URL and the most text kept per run.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OutputSettings {
    /// `file:///path` or `s3://bucket/prefix`; `None` keeps output in `output` beside a SQLite control store.
    pub store: Option<String>,
    pub max_bytes_per_run: Option<u64>,
}

/// One process stream.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum OutputStream {
    Stdout,
    Stderr,
}

impl OutputStream {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Stdout => crate::run_output::constants::STDOUT_STREAM,
            Self::Stderr => crate::run_output::constants::STDERR_STREAM,
        }
    }
}

/// What a worker printed in one batch window, a segment per node and stream in the order printed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct OutputBatch {
    /// The worker's batch number within its claim, from 0; a retried batch is stored once.
    pub batch: u64,
    pub segments: Vec<OutputSegment>,
}

/// Complete lines one node printed to one stream, each ending in a newline.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct OutputSegment {
    /// The node that printed the lines, keyed `<kind>:<name>`; `None` for lines printed outside a node.
    pub node_key: Option<String>,
    pub stream: OutputStream,
    pub text: String,
    /// Bytes the worker dropped just before these lines because it could not send them fast enough.
    #[serde(default)]
    pub dropped_bytes: u64,
}

/// Whether the server stored a batch; a retry of a stored batch is not stored again.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct OutputBatchReceipt {
    pub stored: bool,
}

/// What a run printed, per node and stream, without its text; and what a size cap left out.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RunOutputIndex {
    pub streams: Vec<OutputStreamSummary>,
    /// Bytes and lines past `max_bytes_per_run` that were dropped between the run's first and last parts.
    pub omitted_bytes: i64,
    pub omitted_lines: i64,
    /// Bytes the worker dropped because it could not send them fast enough.
    pub dropped_bytes: i64,
    /// Lines the run's nodes logged with `ctx.log`, oldest first.
    pub logs: Vec<NodeLogLine>,
}

/// One node's stream: how many lines and bytes it printed, and its error and warning lines.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct OutputStreamSummary {
    /// The node key, or `run` for lines printed outside any node.
    pub node_key: String,
    pub stream: OutputStream,
    pub line_count: i64,
    pub text_bytes: i64,
    /// Numbers of lines that look like errors, such as a traceback, at most 10,000.
    pub error_lines: Vec<i64>,
    /// Numbers of lines that look like warnings, at most 10,000.
    pub warning_lines: Vec<i64>,
}

/// One line a node logged with `ctx.log`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct NodeLogLine {
    pub node_key: String,
    pub level: String,
    pub message: String,
    pub logged_at: chrono::DateTime<chrono::Utc>,
}

/// A page of one node's stream, read by line number.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct OutputPageQuery {
    /// The node key, or `run`.
    pub node: String,
    pub stream: OutputStream,
    /// The first line to read, from 1; leave it out with `tail` to read the stream's last lines.
    pub from: Option<i64>,
    /// How many lines to read, at most 2,000.
    pub limit: Option<u32>,
    /// Read the stream's last `limit` lines.
    #[serde(default)]
    pub tail: bool,
}

/// Lines of one node's stream, and the pages before and after them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct OutputPage {
    pub lines: Vec<OutputLine>,
    /// The stream's line count.
    pub line_count: i64,
    /// The first line of the next page, when there is one.
    pub next: Option<i64>,
    /// The first line of the previous page, when there is one.
    pub previous: Option<i64>,
}

/// One stored line; lines a size cap left out just before it are counted on it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct OutputLine {
    pub number: i64,
    pub text: String,
    #[serde(
        default,
        skip_serializing_if = "crate::run_output::_helpers::chunks::is_zero"
    )]
    pub omitted_before: i64,
}

/// A search of one node's stream for text, case-insensitively.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct OutputSearchQuery {
    pub node: String,
    pub stream: OutputStream,
    pub text: String,
    /// Search lines after this one; a search hands back where to continue.
    pub after: Option<i64>,
    /// How many matches to return, at most 200.
    pub limit: Option<u32>,
}

/// Matching lines in order, and the line after which to continue when the search stopped early.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct OutputSearch {
    pub matches: Vec<OutputLine>,
    pub continue_after: Option<i64>,
}

/// A whole node's stream to download.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct OutputDownloadQuery {
    pub node: String,
    pub stream: OutputStream,
}

/// A batch ready to index: its chunk's key and sizes, and each segment's place in the chunk.
#[derive(Clone, Debug)]
pub(crate) struct PreparedChunk {
    pub(crate) run_id: uuid::Uuid,
    pub(crate) claim_id: uuid::Uuid,
    pub(crate) batch: u64,
    pub(crate) object_key: String,
    pub(crate) text_bytes: i64,
    pub(crate) stored_bytes: i64,
    pub(crate) dropped_bytes: i64,
    pub(crate) segments: Vec<PreparedSegment>,
}

/// One segment of a prepared chunk; its error and warning lines count from 0 within the segment.
#[derive(Clone, Debug)]
pub(crate) struct PreparedSegment {
    pub(crate) node_key: String,
    pub(crate) stream: OutputStream,
    pub(crate) line_count: i64,
    pub(crate) text_offset: i64,
    pub(crate) text_bytes: i64,
    pub(crate) error_offsets: Vec<i64>,
    pub(crate) warning_offsets: Vec<i64>,
}

/// One stored segment of a stream, with the chunk that holds it.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct StoredSegment {
    pub(crate) first_line: i64,
    pub(crate) text_offset: i64,
    pub(crate) text_bytes: i64,
    pub(crate) object_key: String,
    pub(crate) chunk_text_bytes: i64,
}

/// A stream's totals and marked lines as the index keeps them.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct StoredStream {
    pub(crate) node_key: String,
    pub(crate) stream: OutputStream,
    pub(crate) line_count: i64,
    pub(crate) text_bytes: i64,
    pub(crate) error_lines: serde_json::Value,
    pub(crate) warning_lines: serde_json::Value,
}

/// A run's next chunk number and the bytes kept in its first and last parts.
#[derive(Clone, Copy, Debug, Deserialize)]
pub(crate) struct StoredTotals {
    pub(crate) next_chunk: i64,
    pub(crate) head_bytes: i64,
    pub(crate) tail_bytes: i64,
}

/// The oldest chunk of a capped run's last part, which goes first.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct TailChunk {
    pub(crate) chunk: i64,
    pub(crate) text_bytes: i64,
    pub(crate) object_key: String,
    pub(crate) lines: i64,
}

/// A worker's run output on its way to the server: lines assembled per node and stream, sent in batches by one thread.
#[derive(Debug)]
pub struct RunOutputUploader {
    pub(crate) shared: Arc<UploadShared>,
    pub(crate) sender: std::sync::Mutex<Option<std::thread::JoinHandle<()>>>,
}

#[derive(Debug, Default)]
pub(crate) struct UploadShared {
    pub(crate) state: std::sync::Mutex<UploadState>,
    pub(crate) wake: std::sync::Condvar,
}

/// What the capture has handed over and the sender has still to send.
#[derive(Debug, Default)]
pub(crate) struct UploadState {
    /// The bytes after each node and stream's last newline, keyed by node key, empty outside any node.
    pub(crate) partial: std::collections::BTreeMap<(String, OutputStream), Vec<u8>>,
    pub(crate) segments: Vec<OutputSegment>,
    pub(crate) batch_bytes: usize,
    pub(crate) batch_started: Option<std::time::Instant>,
    pub(crate) queued: std::collections::VecDeque<OutputBatch>,
    pub(crate) next_batch: u64,
    /// Bytes dropped since the last segment, reported on the next one.
    pub(crate) unreported_drops: u64,
    pub(crate) closing: Option<std::time::Instant>,
    pub(crate) summary: UploadSummary,
}

/// How a worker's run output went: batches stored, bytes it could not send, and why sending stopped.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UploadSummary {
    pub stored_batches: u64,
    pub dropped_bytes: u64,
    pub failure: Option<String>,
}

/// A finished run whose output is past its retention.
#[derive(Clone, Copy, Debug, Deserialize)]
pub(crate) struct ExpiredRun {
    pub(crate) run_id: uuid::Uuid,
}
