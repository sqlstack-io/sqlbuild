//! The input versions an execution consumed, recorded with its `succeeded` rows.

use std::collections::HashMap;

use crate::consumption::_helpers::decision::input_key;
use crate::consumption::constants::UNKNOWN_VERSION;
use crate::consumption::models::{
    ConsumedInput, ConsumptionHistory, InputFact, NodeFacts, NodeKey,
};

/// Each distinct direct input with its source hash, upstream execution ID or `unknown`.
pub fn consumed_inputs(
    node: &NodeFacts,
    (nodes, history): (&HashMap<NodeKey, NodeFacts>, &ConsumptionHistory),
    this_build: &HashMap<NodeKey, String>,
) -> Vec<ConsumedInput> {
    let mut inputs: Vec<ConsumedInput> = node
        .inputs
        .iter()
        .map(|input| {
            let (kind, name) = input_key(input);
            let version: Option<String> = match input {
                InputFact::Source { version, .. } => version.clone(),
                InputFact::Node(upstream) => this_build.get(upstream).cloned().or_else(|| {
                    nodes
                        .get(upstream)
                        .and_then(|facts| history.latest_complete(&facts.relation))
                        .map(str::to_owned)
                }),
                InputFact::Opaque(_) => None,
            };
            ConsumedInput {
                kind: kind.to_owned(),
                name,
                version: version.unwrap_or_else(|| UNKNOWN_VERSION.to_owned()),
            }
        })
        .collect();
    inputs.sort();
    inputs.dedup_by(|left, right| left.kind == right.kind && left.name == right.name);
    inputs
}
