//! Follow the executions recent builds read back until every reachable one is in the history.

use std::collections::BTreeSet;

use crate::consumption::_helpers::unread_references::unread_references;
use crate::consumption::_helpers::with_referenced::with_referenced;
use crate::consumption::models::{ConsumptionHistory, ConsumptionRow, NodeFacts, RelationKey};

/// `history` plus every older execution its builds read, asking `read` for each reference once.
pub fn read_back_history(
    history: ConsumptionHistory,
    nodes: &[NodeFacts],
    mut read: impl FnMut(&[(RelationKey, String)]) -> Vec<ConsumptionRow>,
) -> ConsumptionHistory {
    let mut history: ConsumptionHistory = history;
    let mut attempted: BTreeSet<(RelationKey, String)> = BTreeSet::new();
    loop {
        let mut unread: Vec<(RelationKey, String)> = Vec::new();
        for (relation, ids) in unread_references(&history, nodes) {
            for id in ids {
                let reference: (RelationKey, String) = (relation.clone(), id);
                if !attempted.contains(&reference) {
                    unread.push(reference);
                }
            }
        }
        if unread.is_empty() {
            return history;
        }
        let rows: Vec<ConsumptionRow> = read(&unread);
        attempted.extend(unread);
        history = with_referenced(history, &rows);
    }
}
