use crate::progress::models::ExpectationStep;
use crate::test_command::models::{
    TestFailureFacts, TestInspectionEntry, TestParameter, TestPlanWarning,
};

pub(super) struct LabelTestCase {
    pub(super) description: &'static str,
    pub(super) source_path: Option<&'static str>,
    pub(super) parameters: Vec<TestParameter>,
    pub(super) expected_label: &'static str,
}

pub(super) struct GroupTestCase {
    pub(super) description: &'static str,
    pub(super) chain: Vec<(String, bool)>,
    pub(super) expected_group: &'static str,
}

pub(super) struct InspectionTestCase {
    pub(super) description: &'static str,
    pub(super) entries: Vec<TestInspectionEntry>,
    pub(super) warnings: Vec<TestPlanWarning>,
    pub(super) expected_text: &'static str,
    pub(super) expected_exit_code: i32,
}

pub(super) struct OutcomeTestCase {
    pub(super) description: &'static str,
    pub(super) outcome: &'static str,
    pub(super) expected_status: &'static str,
}

pub(super) struct WidthTestCase {
    pub(super) description: &'static str,
    pub(super) labels: Vec<(String, String)>,
    pub(super) child_names: Vec<String>,
    pub(super) expected_width: usize,
}

pub(super) struct SummaryTestCase {
    pub(super) description: &'static str,
    pub(super) step: ExpectationStep,
    pub(super) expected_summary: &'static str,
}

pub(super) struct EntryTestCase {
    pub(super) description: &'static str,
    pub(super) facts: TestFailureFacts,
    pub(super) expected_text: &'static str,
    pub(super) expected_reason: &'static str,
}

pub(super) struct ExitTestCase {
    pub(super) description: &'static str,
    pub(super) counts: (usize, usize),
    pub(super) expected_exit_code: i32,
}
