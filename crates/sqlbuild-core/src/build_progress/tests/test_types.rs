use crate::build_progress::models::{
    BatchProgressText, BuildResultText, ModelResultText, NameWidthInputs, PythonNodeText,
    RunningNodeText, SchedulerStateText, SpinnerRow, TestResultText,
};

pub(super) struct FooterTestCase {
    pub(super) description: &'static str,
    pub(super) result: BuildResultText,
    pub(super) python: Vec<PythonNodeText>,
    pub(super) use_color: bool,
    pub(super) expected_text: &'static str,
}

pub(super) struct ModelRowTestCase {
    pub(super) description: &'static str,
    pub(super) model: ModelResultText,
    pub(super) tests: Vec<TestResultText>,
    pub(super) verbose: bool,
    pub(super) expected_text: &'static str,
}

pub(super) struct FailedTestTestCase {
    pub(super) description: &'static str,
    pub(super) test: TestResultText,
    pub(super) expected_text: &'static str,
}

pub(super) struct SpinnerTestCase {
    pub(super) description: &'static str,
    pub(super) row: SpinnerRow,
    pub(super) columns: usize,
    pub(super) expected_text: &'static str,
}

pub(super) struct NameWidthTestCase {
    pub(super) description: &'static str,
    pub(super) inputs: NameWidthInputs,
    pub(super) expected_width: usize,
}

pub(super) struct SchedulerTestCase {
    pub(super) description: &'static str,
    pub(super) state: SchedulerStateText,
    pub(super) expected_text: &'static str,
}

pub(super) struct InlineErrorTestCase {
    pub(super) description: &'static str,
    pub(super) error: (Option<&'static str>, &'static str, Option<&'static str>),
    pub(super) width: usize,
    pub(super) expected_lines: &'static [&'static str],
}

pub(super) struct BatchProgressTestCase {
    pub(super) description: &'static str,
    pub(super) batch: BatchProgressText,
    pub(super) expected_text: &'static str,
}

pub(super) struct HeartbeatTestCase {
    pub(super) description: &'static str,
    pub(super) running: Vec<RunningNodeText>,
    pub(super) expected_text: &'static str,
}

pub(super) struct HeartbeatIntervalTestCase {
    pub(super) description: &'static str,
    pub(super) beats: usize,
    pub(super) expected_seconds: u64,
}
