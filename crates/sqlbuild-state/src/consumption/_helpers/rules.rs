//! The decision rules `--changed` applies; tests swap single rules to prove the simulation notices.

use crate::consumption::models::ForcedRun;

/// Each rule of the decision, with the shipped behaviour as its default.
pub(crate) trait DecisionRules {
    /// An execution's time: the earliest of its rows, so a late duplicate cannot make it newer.
    fn execution_time(&self, earliest: i64, latest: i64) -> i64 {
        let _ = latest;
        earliest
    }

    /// Whether upstream model and seed inputs compare their recorded execution IDs.
    fn compares_node_inputs(&self) -> bool {
        true
    }

    /// Whether a stale upstream, selected or not, makes its downstream run.
    fn follows_upstream(&self) -> bool {
        true
    }

    /// Whether the planner's own reason to run a node is honoured.
    fn honours(&self, forced: &ForcedRun) -> bool {
        let _ = forced;
        true
    }
}

/// The shipped rules.
pub(crate) struct Shipped;

impl DecisionRules for Shipped {}
