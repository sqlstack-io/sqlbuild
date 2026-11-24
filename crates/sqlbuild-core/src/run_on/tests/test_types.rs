use crate::run_on::models::{RunOn, RunOnDecision, RunOnLayers};

pub(super) struct ResolveTestCase {
    pub(super) description: &'static str,
    pub(super) layers: RunOnLayers,
    pub(super) expected: RunOn,
}

pub(super) struct RenderTestCase {
    pub(super) description: &'static str,
    pub(super) decisions: Vec<RunOnDecision>,
    pub(super) expected_text: &'static str,
}
