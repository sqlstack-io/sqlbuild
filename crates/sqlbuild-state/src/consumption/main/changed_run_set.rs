//! What `--changed` does with each selected node.

use crate::consumption::_helpers::decision::decide;
use crate::consumption::models::{
    ChangedOutcome, ChangedPolicy, ConsumptionHistory, NodeFacts, NodeKey,
};

/// Each selected node in order: why it runs, that it is unchanged, or why it is held back.
pub fn changed_run_set(
    nodes: &[NodeFacts],
    history: &ConsumptionHistory,
    policy: &ChangedPolicy,
) -> Vec<(NodeKey, ChangedOutcome)> {
    decide(nodes, history, policy)
}
