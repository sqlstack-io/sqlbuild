//! Older executions that recent builds read, so a rebuilt upstream's definitions can be compared.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::consumption::_helpers::decision::node_input_name;
use crate::consumption::constants::NODE_INPUT;
use crate::consumption::models::{ConsumptionHistory, NodeFacts, RelationKey};

/// Upstream executions that builds in `history` read but `history` lacks, by the upstream's relation.
pub(crate) fn unread_references(
    history: &ConsumptionHistory,
    nodes: &[NodeFacts],
) -> BTreeMap<RelationKey, BTreeSet<String>> {
    let relation_of: HashMap<String, &RelationKey> = nodes
        .iter()
        .map(|node| (node_input_name(&node.key), &node.relation))
        .collect();
    let mut unread: BTreeMap<RelationKey, BTreeSet<String>> = BTreeMap::new();
    for execution in history.all_executions() {
        for input in execution
            .inputs
            .iter()
            .filter(|input| input.kind == NODE_INPUT)
        {
            let Some(relation) = relation_of.get(&input.name) else {
                continue;
            };
            if history.execution(relation, &input.version).is_none() {
                unread
                    .entry((*relation).clone())
                    .or_default()
                    .insert(input.version.clone());
            }
        }
    }
    unread
}
