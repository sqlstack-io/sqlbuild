//! Test cases of the run output tests.

use crate::run_output::models::{FailureEntry, HeaderCause, ProgressFooter, RunCounts, RunHeader};

pub(super) struct FooterTestCase {
    pub(super) description: &'static str,
    pub(super) footer: ProgressFooter,
    pub(super) columns: usize,
    pub(super) expected_lines: Vec<&'static str>,
}

pub(super) struct HeartbeatTestCase {
    pub(super) description: &'static str,
    pub(super) footer: ProgressFooter,
    pub(super) expected_lines: Vec<&'static str>,
}

pub(super) struct FailuresTestCase {
    pub(super) description: &'static str,
    pub(super) entries: Vec<FailureEntry>,
    pub(super) columns: usize,
    pub(super) expected_text: &'static str,
}

pub(super) struct FailedListTestCase {
    pub(super) description: &'static str,
    pub(super) failed: Vec<(String, String)>,
    pub(super) verbose: bool,
    pub(super) expected_text: &'static str,
}

pub(super) struct CompletionTestCase {
    pub(super) description: &'static str,
    pub(super) counts: RunCounts,
    pub(super) details: Vec<String>,
    pub(super) expected_text: &'static str,
}

pub(super) struct HeaderTestCase {
    pub(super) description: &'static str,
    pub(super) header: RunHeader,
    pub(super) columns: usize,
    pub(super) expected_text: &'static str,
}

pub(super) struct CutTestCase {
    pub(super) description: &'static str,
    pub(super) text: &'static str,
    pub(super) width: usize,
    pub(super) expected_text: &'static str,
}

pub(super) struct ReasonTestCase {
    pub(super) description: &'static str,
    pub(super) code: Option<&'static str>,
    pub(super) message: &'static str,
    pub(super) expected_reason: &'static str,
}

pub(super) struct CauseLineTestCase {
    pub(super) description: &'static str,
    pub(super) causes: Vec<HeaderCause>,
    pub(super) count: usize,
    pub(super) expected_rollup: &'static str,
    pub(super) expected_note: &'static str,
}
