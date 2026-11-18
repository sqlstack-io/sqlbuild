//! The row appended for a relation replaced outside a recorded build.

use crate::consumption::constants::{REPLACED_PHASE, UNKNOWN_VERSION};
use crate::consumption::models::{ConsumptionAppend, NodeKey, RelationKey};

/// The `replaced` row of a new execution that never completes, so `relation` has no completed record.
pub fn replaced_append(
    (node, relation): (NodeKey, RelationKey),
    (execution_id, run_id): (&str, &str),
) -> ConsumptionAppend {
    ConsumptionAppend {
        execution_id: execution_id.to_owned(),
        phase: REPLACED_PHASE.to_owned(),
        run_id: run_id.to_owned(),
        node,
        relation,
        version_hash: UNKNOWN_VERSION.to_owned(),
        input_count: 0,
        input: None,
    }
}
