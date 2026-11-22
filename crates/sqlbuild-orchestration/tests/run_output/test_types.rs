use crate::run_output::models::{OutputMark, OutputRow, OutputSearch, SearchDirection};
use crate::store::tests::store_test_types::Backend;

/// Where a page starts: forwards from a line, backwards from before one, or from the end.
#[derive(Clone, Copy, Default)]
pub(crate) struct PageStart {
    pub(crate) from: Option<i64>,
    pub(crate) before: Option<i64>,
    pub(crate) tail: bool,
}

/// A page to read: node (or the run) and stream, where it starts, and its limit.
pub(crate) struct PageRead {
    pub(crate) node: Option<&'static str>,
    pub(crate) stream: Option<crate::run_output::models::OutputStream>,
    pub(crate) start: PageStart,
    pub(crate) limit: u32,
    pub(crate) expected_rows: Vec<OutputRow>,
    pub(crate) expected_links: (Option<i64>, Option<i64>),
}

pub(crate) struct OutputReadTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    /// Each node as (node key, lines, stdout lines, stderr lines).
    pub(crate) expected_nodes: Vec<(&'static str, i64, i64, i64)>,
    pub(crate) expected_run_lines: i64,
    pub(crate) expected_marks: Vec<OutputMark>,
    pub(crate) pages: Vec<PageRead>,
    /// The node's whole download, and its stderr alone.
    pub(crate) expected_downloads: (&'static str, &'static str),
}

/// A marked line holding a NUL, in a batch with another line after it and a batch after that.
pub(crate) struct OutputNulTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) printed: [&'static str; 2],
    pub(crate) expected_mark: &'static str,
    /// Every line as printed, NUL included.
    pub(crate) expected_lines: [&'static str; 4],
}

pub(crate) struct OutputFoldTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) printed: &'static str,
    pub(crate) start: PageStart,
    pub(crate) limit: u32,
    pub(crate) around: Option<i64>,
    pub(crate) expected_rows: Vec<OutputRow>,
    pub(crate) expected_span: (i64, i64),
}

/// One search to run.
pub(crate) struct SearchRead {
    pub(crate) text: &'static str,
    pub(crate) from: i64,
    pub(crate) direction: SearchDirection,
}

pub(crate) struct OutputSearchTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    /// How many one-line batches the node prints before the fixture's own lines.
    pub(crate) filler_batches: u64,
    pub(crate) searches: Vec<SearchRead>,
    /// What each search finds, in order.
    pub(crate) expected_results: Vec<OutputSearch>,
}

pub(crate) struct OutputCapTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) max_bytes_per_run: u64,
    pub(crate) batches: usize,
    /// Each line read back as (number, lines left out just before it).
    pub(crate) expected_lines: Vec<(i64, i64)>,
    pub(crate) expected_omitted: (i64, i64),
    pub(crate) expected_objects: usize,
    pub(crate) expected_mark_lines: Vec<i64>,
}

pub(crate) struct OutputRetentionTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) ended_days_ago: Option<i64>,
    pub(crate) expected_pruned: usize,
    pub(crate) expected_objects: usize,
    /// Error and warning lines still indexed after the pass.
    pub(crate) expected_marks: usize,
}

pub(crate) struct NodeLogTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_logs: Vec<(&'static str, &'static str, &'static str)>,
}

pub(crate) struct OutputLargeTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) printed_bytes: usize,
    /// The control store's files stay below this many bytes.
    pub(crate) expected_control_bytes_below: u64,
    pub(crate) expected_tail_lines: usize,
}

/// What a pass leaves: runs pruned, objects, chunk rows, marks and the run's output row.
pub(crate) type RetentionState = (usize, usize, usize, usize, usize);

pub(crate) struct OutputRetentionFailureTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    /// After a pass that cannot delete the objects, and after the next one that can.
    pub(crate) expected_states: [RetentionState; 2],
}

pub(crate) struct OutputUploadTestCase {
    pub(crate) description: &'static str,
    /// Each write: the stream and its bytes.
    pub(crate) writes: Vec<(crate::run_output::models::OutputStream, Vec<u8>)>,
    /// Whether the sender has already had a batch refused.
    pub(crate) refused_before: bool,
    /// Batches queued, the text bytes they hold, and how many replacement characters that text has.
    pub(crate) expected_queued: (usize, usize, usize),
}

pub(crate) struct OutputLocationTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_folder: &'static str,
}
