use crate::runs::models::{RunEnding, RunNodeCounts};

pub(crate) struct RunStatusTestCase {
    pub(crate) description: &'static str,
    pub(crate) counts: RunNodeCounts,
    pub(crate) ending: RunEnding,
    pub(crate) expected_status: &'static str,
}
