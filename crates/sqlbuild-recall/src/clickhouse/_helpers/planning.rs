//! What an insert plan knows about each key, and the conflict evidence it records.

use std::collections::{BTreeMap, HashMap};

use crate::clickhouse::_helpers::record_key::record_key;
use crate::clickhouse::constants::UNCHECKED_TABLES;
use crate::clickhouse::models::{InsertPlan, StoredRow};
use crate::records::main::conflict_row::conflict_row;
use crate::records::main::timestamp_micros::timestamp_micros;
use crate::records::models::{RecallRow, RecallValue, RowEnvelope};
use crate::schema::main::table_spec::table_spec;
use crate::schema::models::Table;
use crate::store::models::ConflictEvidence;

/// What one key holds: its first digest, every digest, and an artifact's newest sighting.
#[derive(Clone, Debug)]
pub(crate) struct Held {
    pub(crate) first_hash: String,
    pub(crate) hashes: Vec<String>,
    pub(crate) newest_micros: Option<i64>,
}

impl Held {
    /// A key first written by this batch with `hash`.
    pub(crate) fn fresh(hash: &str) -> Self {
        Self {
            first_hash: hash.to_owned(),
            hashes: vec![hash.to_owned()],
            newest_micros: None,
        }
    }

    /// Whether the newest stored copy was seen before `cutoff`.
    pub(crate) fn seen_before(&self, cutoff: Option<i64>) -> bool {
        self.newest_micros
            .zip(cutoff)
            .is_some_and(|(newest, cutoff)| newest < cutoff)
    }
}

/// A row's physical table name and record key.
pub(crate) fn planned_key(row: &RecallRow) -> (String, String) {
    (table_spec(row.table).name.to_owned(), record_key(row))
}

/// The evidence and `recall_conflicts` row for `row` rejected against what `key` holds.
pub(crate) fn conflict(
    row: &RecallRow,
    (record_key, existing): (String, &Held),
    detected_at: &str,
) -> (ConflictEvidence, RecallRow) {
    let evidence = ConflictEvidence {
        table: row.table,
        record_key,
        existing_sha256: existing.first_hash.clone(),
        rejected_sha256: row.content_hash().unwrap_or_default().to_owned(),
    };
    let recorded = conflict_record(row, &evidence, detected_at);
    (evidence, recorded)
}

/// The keys already stored, by physical table name and record key.
pub(crate) fn held_keys(stored: &[StoredRow]) -> HashMap<(String, String), Held> {
    let mut held: HashMap<(String, String), Held> = HashMap::with_capacity(stored.len());
    for row in stored {
        held.insert(
            (row.dataset.clone(), row.record_key.clone()),
            Held {
                first_hash: row.first_hash.clone(),
                hashes: row.hashes.clone(),
                newest_micros: row.newest_seen_at.as_deref().and_then(timestamp_micros),
            },
        );
    }
    held
}

fn text_of(row: &RecallRow, column: &str) -> String {
    match row.value(column) {
        Some(RecallValue::Text(text)) => text.clone(),
        _ => String::new(),
    }
}

/// The `recall_conflicts` row recording `evidence` against `row`'s invocation.
fn conflict_record(row: &RecallRow, evidence: &ConflictEvidence, detected_at: &str) -> RecallRow {
    let envelope = RowEnvelope {
        invocation_id: text_of(row, "invocation_id"),
        invocation_started_at: text_of(row, "invocation_started_at"),
        producer_version: text_of(row, "producer_version"),
    };
    conflict_row(
        &envelope,
        (table_spec(row.table).name, &evidence.record_key),
        (&evidence.existing_sha256, &evidence.rejected_sha256),
        detected_at,
    )
}

/// Sorts a batch's rows into writes, already-stored copies and conflicts.
pub(crate) struct Planner {
    held: HashMap<(String, String), Held>,
    refresh_micros: Option<i64>,
    detected_at: String,
    delivered: BTreeMap<Table, u64>,
    inserts: BTreeMap<Table, Vec<RecallRow>>,
    plan: InsertPlan,
}

impl Planner {
    pub(crate) fn new(stored: &[StoredRow], (detected_at, refresh_before): (&str, &str)) -> Self {
        Self {
            held: held_keys(stored),
            refresh_micros: timestamp_micros(refresh_before),
            detected_at: detected_at.to_owned(),
            delivered: BTreeMap::new(),
            inserts: BTreeMap::new(),
            plan: InsertPlan::default(),
        }
    }

    /// Whether `row` must be written: unchecked, new, or a stale artifact; `None` when it conflicts and was recorded as such.
    fn admit(&mut self, row: &RecallRow) -> Option<bool> {
        let key: (String, String) = planned_key(row);
        let hash: &str = row.content_hash().unwrap_or_default();
        if UNCHECKED_TABLES.contains(&row.table) {
            return Some(true);
        }
        let Some(known) = self.held.get(&key) else {
            self.held.insert(key, Held::fresh(hash));
            return Some(true);
        };
        if !known.hashes.iter().any(|stored| row.same_content(stored)) {
            let (evidence, recorded) = conflict(row, (key.1.clone(), known), &self.detected_at);
            self.inserts
                .entry(Table::Conflicts)
                .or_default()
                .push(recorded);
            self.plan.outcome.conflicts.push(evidence);
            return None;
        }
        let stale: bool = row.table == Table::Artifacts && known.seen_before(self.refresh_micros);
        if stale {
            self.held.insert(key, Held::fresh(hash));
        }
        Some(stale)
    }

    /// Count `row` as delivered and queue it when it must be written.
    pub(crate) fn add(&mut self, row: RecallRow) {
        let Some(write) = self.admit(&row) else {
            return;
        };
        *self.delivered.entry(row.table).or_default() += 1;
        if write {
            self.inserts.entry(row.table).or_default().push(row);
        }
    }

    pub(crate) fn finish(self) -> InsertPlan {
        let mut plan: InsertPlan = self.plan;
        plan.inserts = self.inserts.into_iter().collect();
        plan.outcome.delivered = self.delivered.into_iter().collect();
        plan
    }
}
