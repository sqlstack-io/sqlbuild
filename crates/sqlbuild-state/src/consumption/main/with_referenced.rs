//! Add executions read back by ID to the history `--changed` decides from.

use crate::consumption::_helpers::history::executions_of;
use crate::consumption::models::{ConsumptionHistory, ConsumptionRow};

/// `history` with the executions in `rows` added as read-back executions.
pub fn with_referenced(
    mut history: ConsumptionHistory,
    rows: &[ConsumptionRow],
) -> ConsumptionHistory {
    for (relation, execution) in executions_of(rows) {
        if history
            .execution(&relation, &execution.execution_id)
            .is_none()
        {
            history
                .referenced
                .insert((relation, execution.execution_id.clone()), execution);
        }
    }
    history
}
