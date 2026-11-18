//! Response bodies are serialized once per view, off the async workers, and tagged with the view's snapshot and version.

use std::sync::Arc;

use axum::body::Bytes;

use crate::node_states::models::{CachedJson, NodeStatesRecord, TargetView};
use crate::store::errors::ControlError;
use crate::warehouse_metadata::models::CatalogNodeRecord;

/// The view's node list with each node's state; an empty list before the first publication.
pub(crate) async fn nodes_json(view: &Arc<TargetView>) -> Result<CachedJson, ControlError> {
    let body = view
        .nodes_body
        .get_or_try_init(|| serialized(view.clone(), node_records))
        .await?
        .clone();
    Ok(CachedJson {
        etag: view
            .catalog
            .snapshot_id
            .map(|snapshot_id| format!("\"{snapshot_id}:{}\"", view.version)),
        body,
    })
}

/// The view's states with its snapshot and version.
pub(crate) async fn states_json(view: &Arc<TargetView>) -> Result<CachedJson, ControlError> {
    let body = view
        .states_body
        .get_or_try_init(|| serialized(view.clone(), state_record))
        .await?
        .clone();
    Ok(CachedJson { etag: None, body })
}

async fn serialized<T: serde::Serialize + 'static>(
    view: Arc<TargetView>,
    record: fn(&TargetView) -> T,
) -> Result<Bytes, ControlError> {
    tokio::task::spawn_blocking(move || {
        serde_json::to_vec(&record(&view))
            .map(Bytes::from)
            .map_err(|error| ControlError::Invalid(error.to_string()))
    })
    .await?
}

fn node_records(view: &TargetView) -> Vec<CatalogNodeRecord> {
    view.catalog
        .records
        .iter()
        .map(|record| {
            let mut record = record.clone();
            if let Some(state) = view.states.get(&record.node_key) {
                record.last_build.clone_from(&state.last_build);
                record.last_outcome.clone_from(&state.last_outcome);
                record.definition_changed = state.definition_changed;
                record.checks.clone_from(&state.checks);
            }
            record
        })
        .collect()
}

fn state_record(view: &TargetView) -> NodeStatesRecord {
    NodeStatesRecord {
        snapshot_id: view.catalog.snapshot_id,
        version: view.version,
        states: (*view.states).clone(),
        changed_build: view.changed_build.as_deref().cloned(),
    }
}
