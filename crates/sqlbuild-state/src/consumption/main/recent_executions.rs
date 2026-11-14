//! Choose the recent executions of every relation from rows already read.

use std::collections::{BTreeMap, BTreeSet};

use crate::consumption::models::{ConsumptionRow, RelationKey};

/// Rows of the newest `limit` executions per relation, timing each execution by its earliest row,
/// as `recent_executions_sql` selects them in the warehouse.
pub fn recent_executions(rows: &[ConsumptionRow], limit: usize) -> Vec<ConsumptionRow> {
    let mut started: BTreeMap<(&RelationKey, &str), i64> = BTreeMap::new();
    for row in rows {
        let entry = started
            .entry((&row.row.relation, row.row.execution_id.as_str()))
            .or_insert(row.recorded_at);
        *entry = (*entry).min(row.recorded_at);
    }
    let mut by_relation: BTreeMap<&RelationKey, Vec<(i64, &str)>> = BTreeMap::new();
    for ((relation, execution), time) in started {
        by_relation
            .entry(relation)
            .or_default()
            .push((time, execution));
    }
    let mut kept: BTreeSet<(&RelationKey, &str)> = BTreeSet::new();
    for (relation, executions) in &mut by_relation {
        executions.sort_by(|left, right| right.cmp(left));
        kept.extend(
            executions
                .iter()
                .take(limit)
                .map(|(_, execution)| (*relation, *execution)),
        );
    }
    rows.iter()
        .filter(|row| kept.contains(&(&row.row.relation, row.row.execution_id.as_str())))
        .cloned()
        .collect()
}
