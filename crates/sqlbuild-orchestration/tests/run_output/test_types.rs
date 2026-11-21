use crate::store::tests::store_test_types::Backend;

/// One stream as (node key, stream, lines, error lines, warning lines).
pub(crate) type StreamSummary<N> = (N, &'static str, i64, Vec<i64>, Vec<i64>);

pub(crate) struct OutputReadTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    /// Each stream as (node key, stream, lines, error lines, warning lines).
    pub(crate) expected_streams: Vec<StreamSummary<&'static str>>,
    pub(crate) expected_page: Vec<(i64, &'static str)>,
    pub(crate) expected_page_links: (Option<i64>, Option<i64>),
    pub(crate) expected_tail: Vec<i64>,
    pub(crate) expected_matches: Vec<i64>,
    pub(crate) expected_download: &'static str,
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
}

pub(crate) struct OutputRetentionTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) ended_days_ago: Option<i64>,
    pub(crate) expected_pruned: usize,
    pub(crate) expected_objects: usize,
}

pub(crate) struct NodeLogTestCase {
    pub(crate) description: &'static str,
    pub(crate) backend: Backend,
    pub(crate) expected_logs: Vec<(&'static str, &'static str, &'static str)>,
}
