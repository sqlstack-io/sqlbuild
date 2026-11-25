//! `read_run_results`: what one recorded run attempted, didn't finish and skipped, and the audits that failed it.

use std::collections::BTreeSet;

use crate::store::_helpers::sqlite::checked;
use crate::store::errors::StoreError;
use crate::store::models::{RecallRunResults, SqliteStore};

/// The resources `run_id` attempted or skipped by outcome, with its unignored error-severity audit failures as `audit:<name>`.
pub fn read_run_results(store: &SqliteStore, run_id: &str) -> Result<RecallRunResults, StoreError> {
    let keys = |sql: &str| -> Result<BTreeSet<String>, StoreError> {
        let mut statement = checked(store.connection.prepare(sql), "read the run")?;
        checked(
            statement
                .query_map([run_id], |row| row.get(0))
                .and_then(Iterator::collect),
            "read the run",
        )
    };
    Ok(RecallRunResults {
        run_id: run_id.to_owned(),
        selected: keys(
            "SELECT DISTINCT resource_id FROM recall_events WHERE run_id = ?1 \
             AND resource_id IS NOT NULL AND event_type LIKE 'resource_attempt_%'",
        )?,
        failed: keys(
            "SELECT DISTINCT resource_id FROM recall_events WHERE run_id = ?1 \
             AND event_type = 'resource_attempt_failed' AND resource_id NOT IN \
             (SELECT resource_id FROM recall_events WHERE run_id = ?1 \
              AND event_type = 'resource_attempt_completed' AND resource_id IS NOT NULL) \
             UNION SELECT DISTINCT 'audit:' || audit_name FROM audit_results \
             WHERE run_id = ?1 AND outcome = 'error' AND ignored = 0",
        )?,
        skipped: keys(
            "SELECT DISTINCT resource_id FROM recall_events WHERE run_id = ?1 \
             AND event_type = 'resource_attempt_skipped' AND resource_id IS NOT NULL",
        )?,
    })
}
