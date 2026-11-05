//! Idempotent row writes: insert once, compare content on a duplicate key, record conflicts.

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};

use rusqlite::{Connection, OptionalExtension};

use crate::records::main::conflict_row::conflict_row;
use crate::records::models::{RecallRow, RecallValue, RowEnvelope};
use crate::schema::main::table_spec::table_spec;
use crate::schema::models::Table;
use crate::store::_helpers::schema::stored_columns;
use crate::store::_helpers::sqlite::{SqlValue, checked};
use crate::store::errors::StoreError;
use crate::store::main::conflict_evidence::conflict_evidence;
use crate::store::main::relational_insert::relational_insert;
use crate::store::main::relational_lookup::relational_lookup;
use crate::store::models::{ConflictEvidence, InsertOutcome};

fn text_of(row: &RecallRow, column: &str) -> String {
    row.value(column)
        .map_or_else(String::new, RecallValue::key_text)
}

/// Insert one row's values for the columns its table has; `Some(evidence)` when its key holds different content.
fn write_row(
    connection: &Connection,
    (row, columns): (&RecallRow, &HashSet<String>),
    ingested_at: &str,
) -> Result<Option<ConflictEvidence>, StoreError> {
    let ingested: RecallValue = RecallValue::Text(ingested_at.to_owned());
    let insert = relational_insert((row, columns), &ingested, false);
    let parameters: Vec<SqlValue<'_>> = insert
        .values
        .iter()
        .map(|(value, _)| SqlValue(value))
        .collect();
    let mut statement = checked(
        connection.prepare_cached(&insert.sql),
        "prepare Recall insert",
    )?;
    let changed: usize = checked(
        statement.execute(rusqlite::params_from_iter(parameters.iter())),
        "write Recall rows",
    )?;
    if changed > 0 {
        return Ok(None);
    }
    let lookup = relational_lookup(row, false, false);
    let key_values: Vec<SqlValue<'_>> = lookup
        .values
        .iter()
        .map(|(value, _)| SqlValue(value))
        .collect();
    let mut statement = checked(
        connection.prepare_cached(&lookup.sql),
        "prepare Recall lookup",
    )?;
    let existing: Option<String> = checked(
        statement
            .query_row(rusqlite::params_from_iter(key_values.iter()), |found| {
                found.get(0)
            })
            .optional(),
        "read stored Recall row",
    )?;
    Ok(existing.and_then(|existing| conflict_evidence(row, &existing)))
}

/// Write `rows` on the current transaction; conflicts become `recall_conflicts` rows.
pub(crate) fn write_rows(
    connection: &Connection,
    rows: &[RecallRow],
    ingested_at: &str,
) -> Result<InsertOutcome, StoreError> {
    let mut outcome = InsertOutcome::default();
    let mut columns: HashMap<Table, HashSet<String>> = HashMap::new();
    for row in rows.iter().map(|row| row.table).chain([Table::Conflicts]) {
        if let Entry::Vacant(entry) = columns.entry(row) {
            entry.insert(stored_columns(connection, row)?);
        }
    }
    for row in rows {
        match write_row(connection, (row, &columns[&row.table]), ingested_at)? {
            None => match outcome
                .delivered
                .iter_mut()
                .find(|(seen, _)| *seen == row.table)
            {
                Some((_, count)) => *count += 1,
                None => outcome.delivered.push((row.table, 1)),
            },
            Some(evidence) => {
                let envelope = RowEnvelope {
                    invocation_id: text_of(row, "invocation_id"),
                    invocation_started_at: text_of(row, "invocation_started_at"),
                    producer_version: text_of(row, "producer_version"),
                };
                let conflict = conflict_row(
                    &envelope,
                    (table_spec(row.table).name, &evidence.record_key),
                    (&evidence.existing_sha256, &evidence.rejected_sha256),
                    ingested_at,
                );
                if row.table != Table::Conflicts {
                    write_row(
                        connection,
                        (&conflict, &columns[&Table::Conflicts]),
                        ingested_at,
                    )?;
                }
                outcome.conflicts.push(evidence);
            }
        }
    }
    Ok(outcome)
}
