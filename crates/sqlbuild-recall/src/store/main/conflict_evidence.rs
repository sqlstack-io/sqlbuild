//! Duplicate rows preserve the first content and expose the rejected digest.

use crate::records::models::{RecallRow, RecallValue};
use crate::schema::main::table_spec::table_spec;
use crate::store::models::ConflictEvidence;

/// Evidence only when a stored row's content differs, independent of the storage driver.
#[must_use]
pub fn conflict_evidence(row: &RecallRow, existing: &str) -> Option<ConflictEvidence> {
    let rejected = row.content_hash().unwrap_or_default();
    (existing != rejected).then(|| ConflictEvidence {
        table: row.table,
        record_key: table_spec(row.table)
            .key
            .iter()
            .map(|name| {
                row.value(name)
                    .map_or_else(String::new, RecallValue::key_text)
            })
            .collect::<Vec<_>>()
            .join(":"),
        existing_sha256: existing.to_owned(),
        rejected_sha256: rejected.to_owned(),
    })
}
