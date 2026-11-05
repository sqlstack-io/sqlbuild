//! The recent executions of each relation, rebuilt from rows read back.

use crate::consumption::_helpers::history::build_history;
use crate::consumption::models::{ConsumptionHistory, ConsumptionRow};

/// Executions per relation from rows read with at most `limit` executions per relation.
pub fn consumption_history(rows: &[ConsumptionRow], limit: usize) -> ConsumptionHistory {
    build_history(rows, limit)
}
