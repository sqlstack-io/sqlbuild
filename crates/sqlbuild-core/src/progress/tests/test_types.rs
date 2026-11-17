use crate::cli_text::models::CliStyle;
use crate::progress::models::{ProgressEvent, ProgressSettings, StatementSummary, StatementTiming};
use crate::terminal_events::models::TerminalRequest;

pub(super) struct ProjectionTestCase {
    pub(super) description: &'static str,
    pub(super) settings: ProgressSettings,
    pub(super) enriched: Vec<&'static str>,
    pub(super) events: Vec<ProgressEvent>,
    pub(super) claims: Vec<TerminalRequest>,
    pub(super) expected_lines: Vec<&'static str>,
    pub(super) expected_durations: Vec<Option<f64>>,
}

pub(super) struct CompletionMessageTestCase {
    pub(super) description: &'static str,
    pub(super) message: &'static str,
    pub(super) expected_is_completion: bool,
}

pub(super) struct ExpectationDetailTestCase {
    pub(super) description: &'static str,
    pub(super) step: crate::progress::models::ExpectationStep,
    pub(super) expected_detail: &'static str,
}

pub(super) struct InspectionRecordTestCase {
    pub(super) description: &'static str,
    pub(super) record: crate::progress::models::InspectionRecord,
    pub(super) expected_line: String,
}

pub(super) struct ProgressLineTestCase {
    pub(super) description: &'static str,
    pub(super) line: String,
    pub(super) expected_line: &'static str,
}

pub(super) struct OutputModeTestCase {
    pub(super) description: &'static str,
    pub(super) facts: crate::progress::models::OutputModeFacts,
    pub(super) expected_machine_output: bool,
}

pub(super) struct StatusSpinnerLineTestCase {
    pub(super) description: &'static str,
    pub(super) message: &'static str,
    pub(super) columns: usize,
    pub(super) style: CliStyle,
    pub(super) expected_line: &'static str,
}

pub(super) struct AuditRowTestCase {
    pub(super) description: &'static str,
    pub(super) resource_id: &'static str,
    pub(super) terminal_event: &'static str,
    pub(super) recorded_outcomes: Vec<(&'static str, &'static str)>,
    pub(super) expected_lines: Vec<&'static str>,
}

pub(super) struct StatementCountTestCase {
    pub(super) description: &'static str,
    pub(super) events: Vec<ProgressEvent>,
    pub(super) expected_by_kind: Vec<(&'static str, u64)>,
    pub(super) expected_query_ids: Vec<&'static str>,
}

pub(super) struct StatementSummaryTextTestCase {
    pub(super) description: &'static str,
    pub(super) summary: StatementSummary,
    pub(super) timing: StatementTiming,
    pub(super) expected_line: Option<&'static str>,
    pub(super) expected_json: serde_json::Value,
}
