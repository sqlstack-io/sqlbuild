use std::time::Duration;

use crate::screen_text::models::{
    PhaseLayout, PhaseLine, ReadyCard, ScreenFacts, TreeGroup, WarehouseState,
};

pub(super) struct PhaseLinesTestCase {
    pub(super) description: &'static str,
    pub(super) lines: Vec<PhaseLine>,
    pub(super) layout: PhaseLayout,
    pub(super) facts: ScreenFacts,
    pub(super) expected_rows: Vec<&'static str>,
}

pub(super) struct ReadyCardTestCase {
    pub(super) description: &'static str,
    pub(super) card: ReadyCard,
    pub(super) facts: ScreenFacts,
    pub(super) expected_rows: Vec<&'static str>,
}

pub(super) struct TreeGroupTestCase {
    pub(super) description: &'static str,
    pub(super) group: TreeGroup,
    pub(super) facts: ScreenFacts,
    pub(super) expected_rows: Vec<&'static str>,
}

pub(super) struct ElapsedTestCase {
    pub(super) description: &'static str,
    pub(super) elapsed: Duration,
    pub(super) expected_text: &'static str,
}

pub(super) struct DotTestCase {
    pub(super) description: &'static str,
    pub(super) warehouse: WarehouseState,
    pub(super) expected_dot: &'static str,
}

pub(super) struct EventsTestCase {
    pub(super) description: &'static str,
    pub(super) facts: ScreenFacts,
    pub(super) expected_rows: Vec<&'static str>,
}

pub(super) struct ChromeTestCase {
    pub(super) description: &'static str,
    pub(super) columns: usize,
    pub(super) expected_rows: Vec<&'static str>,
}

pub(super) struct PlainLineTestCase {
    pub(super) description: &'static str,
    pub(super) message: &'static str,
    pub(super) elapsed: Option<Duration>,
    pub(super) expected_line: &'static str,
}

pub(super) struct RedrawTestCase {
    pub(super) description: &'static str,
    pub(super) previous: usize,
    pub(super) rows: Vec<String>,
    pub(super) expected_text: &'static str,
}

pub(super) struct WidthTestCase {
    pub(super) description: &'static str,
    pub(super) colour: bool,
    pub(super) expected_too_wide: Vec<String>,
}
