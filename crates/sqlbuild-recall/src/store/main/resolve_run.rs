//! `resolve_run`: the recorded run a full ID or unique prefix of at least 8 characters names.

use crate::store::_helpers::sqlite::checked;
use crate::store::constants::{LISTED_RUN_CANDIDATES, MIN_RUN_ID_PREFIX};
use crate::store::errors::{StoreError, StoreErrorKind};
use crate::store::models::SqliteStore;

/// The full run ID `selector` names: itself, or the one recorded run it prefixes.
pub fn resolve_run(store: &SqliteStore, selector: &str) -> Result<String, StoreError> {
    if selector.chars().count() < MIN_RUN_ID_PREFIX {
        return Err(StoreError::new(
            StoreErrorKind::Query,
            format!(
                "run ID '{selector}' is too short: give the full ID or at least {MIN_RUN_ID_PREFIX} characters of it"
            ),
        ));
    }
    let mut statement = checked(
        store.connection.prepare(
            "SELECT DISTINCT run_id FROM recall_events \
             WHERE run_id IS NOT NULL AND substr(run_id, 1, length(?1)) = ?1 \
             ORDER BY run_id LIMIT ?2",
        ),
        "find the run",
    )?;
    let candidates: Vec<String> = checked(
        statement
            .query_map(
                rusqlite::params![selector, LISTED_RUN_CANDIDATES + 1],
                |row| row.get(0),
            )
            .and_then(Iterator::collect),
        "find the run",
    )?;
    if let Some(exact) = candidates.iter().find(|candidate| *candidate == selector) {
        return Ok(exact.clone());
    }
    match candidates.as_slice() {
        [only] => Ok(only.clone()),
        [] => Err(StoreError::new(
            StoreErrorKind::NotFound,
            format!("no run '{selector}' in the local Recall history"),
        )),
        several => {
            let listed: &[String] = &several[..several.len().min(LISTED_RUN_CANDIDATES)];
            Err(StoreError::new(
                StoreErrorKind::Ambiguous,
                format!(
                    "run ID '{selector}' is ambiguous: it starts {}{}; give more of the ID",
                    listed.join(", "),
                    if several.len() > listed.len() {
                        ", and more"
                    } else {
                        ""
                    }
                ),
            ))
        }
    }
}
