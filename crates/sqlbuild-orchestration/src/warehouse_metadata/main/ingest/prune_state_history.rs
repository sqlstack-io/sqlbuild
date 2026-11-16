//! Keep the newest builds of each node and results of each check in bounded pages.

use std::collections::BTreeSet;

use crate::store::errors::ControlError;
use crate::store::main::{query::query, write::write};
use crate::store::models::{ControlStore, Parameter};
use crate::warehouse_metadata::constants::STATE_HISTORY_PRUNE_BATCH;
use crate::warehouse_metadata::models::StateHistoryRetention;

/// Builds ranked past the limit within their node, in one state namespace; the newest always stays.
const FINGERPRINTS: &str = "DELETE FROM sqb_warehouse_fingerprints WHERE event_id IN (SELECT event_id FROM \
    (SELECT event_id, ROW_NUMBER() OVER (PARTITION BY location_id, target, state_database, state_schema, \
    node_type, node_name, target_database, target_schema, target_name ORDER BY observed_at DESC, event_id DESC) \
    AS version FROM sqb_warehouse_fingerprints) ranked WHERE version > $1 LIMIT $2) RETURNING definition_text_hash";

/// Check results ranked past the limit within their check binding, in one state namespace.
const AUDIT_RESULTS: &str = "DELETE FROM sqb_warehouse_audit_results WHERE event_id IN (SELECT event_id FROM \
    (SELECT event_id, ROW_NUMBER() OVER (PARTITION BY location_id, target, state_database, state_schema, \
    COALESCE(binding_key, audit_name) ORDER BY observed_at DESC, event_id DESC) AS version \
    FROM sqb_warehouse_audit_results) ranked WHERE version > $1 LIMIT $2) RETURNING event_id";

/// Removes one page of builds and one page of check results past the limit.
///
/// Returns the rows deleted and whether a page came back full, so more may remain. Each page
/// commits on its own, so a repeated or interrupted pass only finds rows still stored.
pub(crate) async fn prune_state_history(
    store: &ControlStore,
    retention: StateHistoryRetention,
) -> Result<(usize, bool), ControlError> {
    let versions = i64::from(retention.kept_versions(store));
    let limit = STATE_HISTORY_PRUNE_BATCH as i64;
    let fingerprints = write(store, |transaction| async move {
        let (mut transaction, rows) = query(
            transaction,
            FINGERPRINTS,
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
            AUDIT_RESULTS,
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
