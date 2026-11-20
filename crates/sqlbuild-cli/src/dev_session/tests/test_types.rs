//! Table rows for the `sqb dev` foreground session.

use sqlbuild_orchestration::runs::models::RunRecord;

use crate::dev_session::_helpers::runs::SeenRuns;

/// A `sqb debug --json` report and the adapter and warehouse state read from it.
pub(super) struct OutcomeTestCase {
    pub(super) description: &'static str,
    pub(super) report: String,
    pub(super) expected_adapter: &'static str,
    pub(super) expected_state: String,
}

/// A stderr chunk from `sqb debug` and how it splits into client text and SQLBuild rows.
pub(super) struct SplitTestCase {
    pub(super) description: &'static str,
    pub(super) chunk: &'static str,
    pub(super) expected_client: &'static str,
    pub(super) expected_rows: Vec<&'static str>,
}

/// One run poll: what was seen before, the runs read newest first, and the event lines it yields.
pub(super) struct RunPollTestCase {
    pub(super) description: &'static str,
    pub(super) seen: Option<SeenRuns>,
    pub(super) runs: Vec<RunRecord>,
    pub(super) expected_messages: Vec<String>,
}

/// `sqb debug` stdout in chunks: the client's text each chunk shows, and the report left.
pub(super) struct ReportSplitTestCase {
    pub(super) description: &'static str,
    pub(super) chunks: Vec<&'static str>,
    pub(super) expected_client: Vec<&'static str>,
    pub(super) expected_report: &'static str,
}
