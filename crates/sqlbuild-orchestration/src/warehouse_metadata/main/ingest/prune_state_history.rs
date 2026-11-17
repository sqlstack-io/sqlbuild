//! Keep the newest builds of each node and results of each check in bounded pages.

use std::collections::BTreeSet;

use crate::store::errors::ControlError;
use crate::store::main::{query::query, write::write};
use crate::store::models::{ControlStore, Parameter};
use crate::warehouse_metadata::constants::{
    PRUNE_AUDIT_RESULTS_SQL, PRUNE_FINGERPRINTS_SQL, STATE_HISTORY_PRUNE_BATCH,
};
use crate::warehouse_metadata::models::StateHistoryRetention;

/// Remove one page of builds and of check results past the limit; returns rows deleted and whether more remain.
pub(crate) async fn prune_state_history(
    store: &ControlStore,
    retention: StateHistoryRetention,
) -> Result<(usize, bool), ControlError> {
    let versions = i64::from(retention.kept_versions(store));
    let limit = STATE_HISTORY_PRUNE_BATCH as i64;
    let fingerprints = write(store, |transaction| async move {
        let (mut transaction, rows) = query(
            transaction,
            PRUNE_FINGERPRINTS_SQL,
            vec![Parameter::Integer(versions), Parameter::Integer(limit)],
        )
        .await?;
        let texts: Vec<Parameter> = rows
            .iter()
            .filter_map(|row| row["definition_text_hash"].as_str())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|hash| Parameter::Text(hash.to_owned()))
            .collect();
        if !texts.is_empty() {
            (transaction, _) = query(
                transaction,
                &format!(
                    "DELETE FROM sqb_source_texts WHERE text_hash IN ({}) \
                     AND NOT EXISTS (SELECT 1 FROM sqb_warehouse_fingerprints f WHERE f.definition_text_hash=sqb_source_texts.text_hash) \
                     AND NOT EXISTS (SELECT 1 FROM sqb_snapshot_sources s WHERE s.text_hash=sqb_source_texts.text_hash)",
                    Parameter::placeholders(1..=texts.len())
                ),
                texts,
            )
            .await?;
        }
        Ok((transaction, rows.len()))
    })
    .await?;
    let results = write(store, |transaction| async move {
        let (transaction, rows) = query(
            transaction,
            PRUNE_AUDIT_RESULTS_SQL,
            vec![Parameter::Integer(versions), Parameter::Integer(limit)],
        )
        .await?;
        Ok((transaction, rows.len()))
    })
    .await?;
    Ok((
        fingerprints + results,
        fingerprints == STATE_HISTORY_PRUNE_BATCH || results == STATE_HISTORY_PRUNE_BATCH,
    ))
}
