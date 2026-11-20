use std::time::Duration;

use crate::screen_text::models::{
    BoxRow, EventLine, Problem, ResultLine, ScreenFacts, StepLine, TreeGroup,
};

pub(super) struct StepLineTestCase {
    pub(super) description: &'static str,
    pub(super) line: StepLine,
    pub(super) lasting: bool,
    pub(super) columns: usize,
    pub(super) expected_rows: Vec<&'static str>,
}

pub(super) struct ProblemTestCase {
    pub(super) description: &'static str,
    pub(super) problem: Problem,
    pub(super) columns: usize,
    pub(super) expected_rows: Vec<&'static str>,
}

pub(super) struct ReadyBoxTestCase {
    pub(super) description: &'static str,
    pub(super) rows: Vec<BoxRow>,
    pub(super) columns: usize,
    pub(super) expected_rows: Vec<&'static str>,
}

pub(super) struct EventTestCase {
    pub(super) description: &'static str,
    pub(super) event: EventLine,
    pub(super) columns: usize,
    pub(super) expected_rows: Vec<&'static str>,
}

pub(super) struct ResultTestCase {
    pub(super) description: &'static str,
    pub(super) result: ResultLine,
    pub(super) columns: usize,
    pub(super) expected_rows: Vec<&'static str>,
}

pub(super) struct PromptTestCase {
    pub(super) description: &'static str,
    pub(super) text: &'static str,
    pub(super) columns: usize,
    pub(super) expected_rows: Vec<&'static str>,
}

pub(super) struct ElapsedTestCase {
    pub(super) description: &'static str,
    pub(super) elapsed: Duration,
    pub(super) expected_text: &'static str,
}

pub(super) struct VisibleCellsTestCase {
    pub(super) description: &'static str,
    pub(super) row: String,
    pub(super) expected_cells: usize,
}

pub(super) struct RedrawTestCase {
    pub(super) description: &'static str,
    pub(super) previous: usize,
    pub(super) rows: Vec<String>,
    pub(super) expected_text: &'static str,
}

pub(super) struct WidthTestCase {
    pub(super) description: &'static str,
    pub(super) facts: fn(usize) -> ScreenFacts,
    pub(super) expected_too_wide: Vec<String>,
}

pub(super) struct TreeGroupTestCase {
    pub(super) description: &'static str,
    pub(super) group: TreeGroup,
    pub(super) facts: ScreenFacts,
    pub(super) expected_rows: Vec<&'static str>,
}
