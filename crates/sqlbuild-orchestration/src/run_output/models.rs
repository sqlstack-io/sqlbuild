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

/// What a run printed, per node, without its text; one node's error and warning lines; and what a size cap left out.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RunOutputIndex {
    pub nodes: Vec<NodeOutputSummary>,
    /// Lines the whole run printed, numbered in the order printed.
    pub line_count: i64,
    /// Bytes and lines past `max_bytes_per_run` that were dropped between the run's first and last parts.
    pub omitted_bytes: i64,
    pub omitted_lines: i64,
    /// Bytes the worker dropped because it could not send them fast enough.
    pub dropped_bytes: i64,
    /// Lines the run's nodes logged with `ctx.log`, oldest first.
    pub logs: Vec<NodeLogLine>,
    /// The named node's lines that look like errors or warnings, by line; empty when no node is named.
    pub marks: Vec<OutputMark>,
}

/// How much one node printed: its lines, numbered across both streams, and its bytes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct NodeOutputSummary {
    /// The node key, or `run` for lines printed outside any node.
    pub node_key: String,
    pub line_count: i64,
    pub stdout_lines: i64,
    pub stderr_lines: i64,
    pub text_bytes: i64,
}

/// A line that looks like an error, such as a traceback, or a warning; each stream keeps at most 10,000.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct OutputMark {
    /// The line in the node's numbering.
    pub line: i64,
    pub stream: OutputStream,
    pub level: OutputLevel,
    /// The line's text, at most 300 bytes.
    pub text: String,
}

/// How a printed line reads: as an error or as a warning.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum OutputLevel {
    Error,
    Warning,
}

impl OutputLevel {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Error => crate::run_output::constants::ERROR_LEVEL,
            Self::Warning => crate::run_output::constants::WARNING_LEVEL,
        }
    }
}

/// One line a node logged with `ctx.log`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct NodeLogLine {
    pub node_key: String,
    pub level: String,
    pub message: String,
    pub logged_at: chrono::DateTime<chrono::Utc>,
}

/// A page of a node's output, numbered across its two streams, or of the whole run's output, numbered in the order printed.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct OutputPageQuery {
    /// The node key, or `run`; leave it out to read the whole run's output.
    pub node: Option<String>,
    /// Only this stream's lines, keeping their numbers.
    pub stream: Option<OutputStream>,
    /// Read forwards from this line, from 1.
    pub from: Option<i64>,
    /// Read the rows that end just before this line.
    pub before: Option<i64>,
    /// Read the last rows.
    #[serde(default)]
    pub tail: bool,
    /// How many rows to read, at most 2,000.
    pub limit: Option<u32>,
    /// Fold four or more lines that are the same but for their numbers into one row.
    #[serde(default)]
    pub collapse: bool,
    /// A line to keep open, with three lines either side, when folding.
    pub around: Option<i64>,
}

/// Rows covering lines `from` up to `to`, and where the pages before and after start.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct OutputPage {
    pub rows: Vec<OutputRow>,
    /// The highest line number of the node, or of the run.
    pub line_count: i64,
    /// The first line the rows cover.
    pub from: i64,
    /// The line after the last the rows cover.
    pub to: i64,
    /// `from` for the next page, when there are later lines.
    pub next: Option<i64>,
    /// `before` for the previous page, when there are earlier lines.
    pub previous: Option<i64>,
}

/// A line, or a run of lines that are the same but for their numbers, folded into one row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OutputRow {
    Line {
        line: OutputLine,
    },
    Similar {
        from: i64,
        count: i64,
        sample: OutputLine,
    },
}

/// One stored line; lines a size cap left out just before it are counted on it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct OutputLine {
    pub number: i64,
    /// The node that printed it, or `run`.
    pub node_key: String,
    pub stream: OutputStream,
    pub text: String,
    pub level: Option<OutputLevel>,
    #[serde(
        default,
        skip_serializing_if = "crate::run_output::_helpers::chunks::is_zero"
    )]
    pub omitted_before: i64,
}

/// Which way a search looks from its line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SearchDirection {
    #[default]
    Forward,
    Backward,
}

/// A search of a node's output for text, case-insensitively, from a line in one direction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct OutputSearchQuery {
    pub node: String,
    pub stream: Option<OutputStream>,
    pub text: String,
    /// Look from this line, which a forward search includes, as a backward one does.
    pub from: Option<i64>,
    #[param(inline)]
    pub direction: Option<SearchDirection>,
}

/// The next match; across the whole output, how many match and which this is, when the output was small enough to scan; or where to continue.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct OutputSearch {
    pub line: Option<i64>,
    pub count: Option<i64>,
    pub rank: Option<i64>,
    /// `from` for the next search in the same direction, when this one stopped before finding a match.
    pub continue_from: Option<i64>,
}

/// A whole node's output to download, both streams or one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct OutputDownloadQuery {
    pub node: String,
    pub stream: Option<OutputStream>,
}

/// A node's lines across both streams, or one stream of them, or the whole run's lines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OutputView {
    pub(crate) run_id: uuid::Uuid,
    pub(crate) node: Option<String>,
    pub(crate) stream: Option<OutputStream>,
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
    /// Each marked line's offset within the segment, level and text.
    pub(crate) marks: Vec<(i64, OutputLevel, String)>,
}

/// One stored segment of a view, numbered in the view's numbering, with the chunk that holds it.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct StoredSegment {
    pub(crate) first_line: i64,
    pub(crate) line_count: i64,
    pub(crate) node_key: String,
    pub(crate) stream: OutputStream,
    pub(crate) text_offset: i64,
    pub(crate) text_bytes: i64,
    pub(crate) object_key: String,
    pub(crate) chunk_text_bytes: i64,
}

/// A node's line count and its marks so far in one stream, before a segment adds to them.
#[derive(Clone, Copy, Debug, Default, Deserialize)]
pub(crate) struct StoredNodeLines {
    pub(crate) node_lines: i64,
    pub(crate) stream_marks: i64,
}

/// A run's next chunk number and the bytes kept in its first and last parts.
#[derive(Clone, Copy, Debug, Deserialize)]
pub(crate) struct StoredTotals {
    pub(crate) next_chunk: i64,
    pub(crate) line_count: i64,
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

/// One count read from the index.
#[derive(Clone, Copy, Debug, Deserialize)]
pub(crate) struct ViewCount {
    pub(crate) count: i64,
}

/// Where a view's kept tail starts after a size cap, and where its kept head ends.
#[derive(Clone, Copy, Debug, Deserialize)]
pub(crate) struct StoredGap {
    pub(crate) first_tail: Option<i64>,
    pub(crate) after_head: Option<i64>,
    pub(crate) omitted_lines: Option<i64>,
}

/// Reads a view's lines one at a time in one direction, a batch of segments at a time, each chunk once.
#[derive(Debug)]
pub(crate) struct ViewReader<'a> {
    pub(crate) store: &'a crate::store::models::ControlStore,
    pub(crate) output: &'a OutputStore,
    pub(crate) view: &'a OutputView,
    pub(crate) direction: SearchDirection,
    /// Forwards, the next line to read; backwards, the line before which to read.
    pub(crate) bound: i64,
    /// The view's first line after a size cap's gap, and how many lines the gap left out.
    pub(crate) gap: Option<(i64, i64)>,
    pub(crate) pending: std::collections::VecDeque<OutputLine>,
    pub(crate) exhausted: bool,
    /// The last chunk read, by object key.
    pub(crate) cached: Option<(String, Arc<String>)>,
    pub(crate) chunks_read: usize,
}

/// A page's rows as they are read, and the lowest and highest lines they cover.
#[derive(Clone, Debug, Default)]
pub(crate) struct PageRows {
    pub(crate) rows: Vec<OutputRow>,
    pub(crate) low: Option<i64>,
    pub(crate) high: Option<i64>,
}

/// Lines read so far that are the same but for their numbers, and what they share.
#[derive(Clone, Debug, Default)]
pub(crate) struct SimilarRun {
    pub(crate) key: Option<String>,
    pub(crate) lines: Vec<OutputLine>,
}

/// A run's line count and what a size cap or a slow worker left out.
#[derive(Clone, Copy, Debug, Default, Deserialize)]
pub(crate) struct StoredOutputTotals {
    pub(crate) line_count: i64,
    pub(crate) omitted_bytes: i64,
    pub(crate) omitted_lines: i64,
    pub(crate) dropped_bytes: i64,
}

/// Which node's error and warning lines the index includes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct OutputIndexQuery {
    pub node: Option<String>,
}

/// A whole search's matches: how many, the first and last, and where they sit around the line searched from.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct SearchTally {
    pub(crate) count: i64,
    /// Matches before the line searched from, and up to and including it.
    pub(crate) before: i64,
    pub(crate) through: i64,
    pub(crate) first: Option<i64>,
    pub(crate) last: Option<i64>,
    /// The first match at or after the line searched from, and the last at or before it.
    pub(crate) after: Option<i64>,
    pub(crate) upto: Option<i64>,
}
