//! The rows appended after a model or seed's write succeeded.

use crate::consumption::constants::SUCCEEDED_PHASE;
use crate::consumption::models::{ConsumedInput, ConsumptionAppend, NodeFacts};

/// One `succeeded` row per consumed input, or one input-less row when there are none.
pub fn succeeded_appends(
    node: &NodeFacts,
    (execution_id, run_id): (&str, &str),
    inputs: &[ConsumedInput],
) -> Vec<ConsumptionAppend> {
    let row = |input: Option<&ConsumedInput>| ConsumptionAppend {
        execution_id: execution_id.to_owned(),
        phase: SUCCEEDED_PHASE.to_owned(),
        run_id: run_id.to_owned(),
        node: node.key.clone(),
        relation: node.relation.clone(),
        version_hash: node.version_hash.clone(),
        input_count: i64::try_from(inputs.len()).unwrap_or(i64::MAX),
        input: input.cloned(),
    };
    if inputs.is_empty() {
        return vec![row(None)];
    }
    inputs.iter().map(|input| row(Some(input))).collect()
}
