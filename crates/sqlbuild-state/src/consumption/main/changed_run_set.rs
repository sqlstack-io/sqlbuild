//! Which selected nodes `--changed` runs.

use crate::consumption::_helpers::decision::decide;
use crate::consumption::models::{ConsumptionHistory, NodeFacts, NodeKey, RunReason};

/// Each selected node in order with why it runs, or None when nothing it or an upstream read changed.
pub fn changed_run_set(
    nodes: &[NodeFacts],
    history: &ConsumptionHistory,
) -> Vec<(NodeKey, Option<RunReason>)> {
    decide(nodes, history)
}
