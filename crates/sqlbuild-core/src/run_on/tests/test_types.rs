use crate::run_on::models::{RunOn, RunOnDecision, RunOnLayers};
use crate::run_output::models::{ExplainRow, HeaderCause};

pub(super) struct ResolveTestCase {
    pub(super) description: &'static str,
    pub(super) layers: RunOnLayers,
    pub(super) expected_run_on: RunOn,
}

pub(super) struct RenderTestCase {
    pub(super) description: &'static str,
    pub(super) decisions: Vec<RunOnDecision>,
    /// Whether the refusals stop the tests running, as under `--run-on local`.
    pub(super) errors: bool,
    pub(super) verbose: bool,
    pub(super) columns: usize,
    pub(super) expected_text: &'static str,
}

pub(super) struct CausesTestCase {
    pub(super) description: &'static str,
    pub(super) decisions: Vec<RunOnDecision>,
    pub(super) expected_causes: Vec<HeaderCause>,
    pub(super) expected_rows: Vec<ExplainRow>,
}

pub(super) struct InspectionTestCase {
    pub(super) description: &'static str,
    pub(super) decision: RunOnDecision,
    pub(super) expected_line: &'static str,
}

pub(super) struct ColumnTestCase {
    pub(super) description: &'static str,
    pub(super) run_on: &'static str,
    pub(super) expected_column: &'static str,
}

pub(super) struct JsonTestCase {
    pub(super) description: &'static str,
    pub(super) decisions: Vec<RunOnDecision>,
    pub(super) expected_checks: serde_json::Value,
    pub(super) expected_summary: serde_json::Value,
    pub(super) expected_refusals: serde_json::Value,
}

pub(super) struct ReadTestCase {
    pub(super) description: &'static str,
    pub(super) runner_json: &'static str,
    pub(super) expected_decisions: Option<Vec<(Option<String>, i64)>>,
}

pub(super) struct LocalRefusalTestCase {
    pub(super) description: &'static str,
    pub(super) mode: RunOn,
    pub(super) expected_decision: RunOnDecision,
}
