use std::time::Duration;

use crate::screen_text::models::{PhaseLayout, PhaseLine, ReadyCard, ScreenFacts, TreeGroup};

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
