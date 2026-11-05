//! Rebuild executions from deduplicated rows and read the window `--changed` compares.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::consumption::constants::{ORDER_SKEW_MICROS, SUCCEEDED_PHASE};
use crate::consumption::models::{
    ConsumedInput, ConsumptionHistory, ConsumptionRow, Execution, RelationHistory, RelationKey,
};

/// The rows of one execution with the earliest and latest times seen for each event.
#[derive(Default)]
struct ExecutionRows<'a> {
    rows: Vec<&'a ConsumptionRow>,
    earliest: i64,
    latest: i64,
}

/// Recent executions per relation, from rows read newest first with at most `limit` per relation.
pub(crate) fn build_history(rows: &[ConsumptionRow], limit: usize) -> ConsumptionHistory {
    let mut events: HashMap<&str, (&ConsumptionRow, i64, i64)> = HashMap::new();
    for row in rows {
        let entry = events.entry(row.row.event_id.as_str()).or_insert((
            row,
            row.recorded_at,
            row.recorded_at,
        ));
        if row.recorded_at < entry.1 {
            *entry = (row, row.recorded_at, entry.2);
        }
        entry.2 = entry.2.max(row.recorded_at);
    }
    let mut grouped: BTreeMap<(&RelationKey, &str), ExecutionRows<'_>> = BTreeMap::new();
    for (row, earliest, latest) in events.into_values() {
        let key = (&row.row.relation, row.row.execution_id.as_str());
        let execution = grouped.entry(key).or_insert(ExecutionRows {
            rows: Vec::new(),
            earliest,
            latest,
        });
        execution.earliest = execution.earliest.min(earliest);
        execution.latest = execution.latest.max(latest);
        execution.rows.push(row);
    }
    let mut relations: HashMap<RelationKey, RelationHistory> = HashMap::new();
    for ((relation, execution_id), execution_rows) in grouped {
        relations
            .entry(relation.clone())
            .or_default()
            .executions
            .push(execution(execution_id, &execution_rows));
    }
    for history in relations.values_mut() {
        history.executions.sort_by(|left, right| {
            (right.started_at, &right.execution_id).cmp(&(left.started_at, &left.execution_id))
        });
        history.truncated = history.executions.len() >= limit;
    }
    ConsumptionHistory { relations }
}

fn execution(execution_id: &str, rows: &ExecutionRows<'_>) -> Execution {
    let succeeded: Vec<&ConsumptionRow> = rows
        .rows
        .iter()
        .copied()
        .filter(|row| row.row.phase == SUCCEEDED_PHASE)
        .collect();
    let versions: BTreeSet<&str> = rows
        .rows
        .iter()
        .map(|row| row.row.version_hash.as_str())
        .collect();
    let counts: BTreeSet<i64> = succeeded.iter().map(|row| row.row.input_count).collect();
    let mut inputs: BTreeMap<(&str, &str), BTreeSet<&str>> = BTreeMap::new();
    let mut inputless: bool = false;
    for row in &succeeded {
        match &row.row.input {
            Some(input) => {
                inputs
                    .entry((input.kind.as_str(), input.name.as_str()))
                    .or_default()
                    .insert(input.version.as_str());
            }
            None => inputless = true,
        }
    }
    let consistent: bool = versions.len() == 1
        && counts.len() == 1
        && inputs.values().all(|versions| versions.len() == 1);
    let expected: Option<i64> = counts.first().copied();
    let complete: bool = consistent
        && match expected {
            Some(0) => inputless && inputs.is_empty(),
            Some(count) => !inputless && i64::try_from(inputs.len()) == Ok(count),
            None => false,
        };
    Execution {
        execution_id: execution_id.to_owned(),
        started_at: rows.earliest,
        complete,
        version_hash: (versions.len() == 1)
            .then(|| versions.first().map(|version| (*version).to_owned()))
            .flatten(),
        inputs: if complete {
            inputs.into_iter().filter_map(single_version).collect()
        } else {
            Vec::new()
        },
    }
}

/// The input with its one recorded version.
fn single_version(
    ((kind, name), versions): ((&str, &str), BTreeSet<&str>),
) -> Option<ConsumedInput> {
    let version: &str = versions.first()?;
    Some(ConsumedInput {
        kind: kind.to_owned(),
        name: name.to_owned(),
        version: version.to_owned(),
    })
}

/// The executions `--changed` must all check: those within the order skew of the newest.
pub(crate) struct Window<'a> {
    pub(crate) executions: &'a [Execution],
    /// Older unread executions may also fall inside the skew.
    pub(crate) ambiguous: bool,
}

impl ConsumptionHistory {
    /// The newest executions of `relation` within the skew, or None when it has none.
    pub(crate) fn window(&self, relation: &RelationKey) -> Option<Window<'_>> {
        let history: &RelationHistory = self.relations.get(relation)?;
        let newest: i64 = history.executions.first()?.started_at;
        let length: usize = history
            .executions
            .iter()
            .take_while(|execution| execution.started_at >= newest - ORDER_SKEW_MICROS)
            .count();
        Some(Window {
            executions: &history.executions[..length],
            ambiguous: history.truncated && length == history.executions.len(),
        })
    }

    /// The execution ID of `relation`'s newest execution when it alone is newest and complete.
    pub fn latest_complete(&self, relation: &RelationKey) -> Option<&str> {
        let window: Window<'_> = self.window(relation)?;
        match window.executions {
            [only] if !window.ambiguous && only.complete => Some(only.execution_id.as_str()),
            _ => None,
        }
    }
}
