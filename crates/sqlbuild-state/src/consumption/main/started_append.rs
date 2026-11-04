//! The row appended before a model or seed writes.

use crate::consumption::_helpers::decision::input_key;
use crate::consumption::constants::STARTED_PHASE;
use crate::consumption::main::event_id::event_id;
use crate::consumption::models::{ConsumptionAppend, NodeFacts};

/// The `started` row of one execution of `node`; its input count is the node's distinct direct inputs.
pub fn started_append(node: &NodeFacts, execution_id: &str, run_id: &str) -> ConsumptionAppend {
    let mut keys: Vec<(&str, String)> = node.inputs.iter().map(input_key).collect();
    keys.sort();
    keys.dedup();
    ConsumptionAppend {
        event_id: event_id(execution_id, STARTED_PHASE, None),
        execution_id: execution_id.to_owned(),
        phase: STARTED_PHASE.to_owned(),
        run_id: run_id.to_owned(),
        node: node.key.clone(),
        relation: node.relation.clone(),
        version_hash: node.version_hash.clone(),
        input_count: i64::try_from(keys.len()).unwrap_or(i64::MAX),
        input: None,
    }
}
