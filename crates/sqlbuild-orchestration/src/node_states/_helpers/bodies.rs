//! Response bodies are serialized once, off the async workers: the node list once per snapshot, which also tags it, and the states once per view.

use std::sync::Arc;

use axum::body::Bytes;

use crate::node_states::models::{CachedJson, NodeStatesRecord, TargetView};
use crate::store::errors::ControlError;

/// The snapshot's node list without states; an empty list before the first publication.
pub(crate) async fn nodes_json(view: &Arc<TargetView>) -> Result<CachedJson, ControlError> {
    let catalog = view.catalog.clone();
    let body = view
        .catalog
        .body
        .get_or_try_init(|| serialized(move || serde_json::to_vec(&catalog.records)))
        .await?
        .clone();
    Ok(CachedJson {
        etag: view
            .catalog
            .snapshot_id
            .map(|snapshot_id| format!("\"{snapshot_id}\"")),
        body,
    })
}

/// The view's states with its snapshot and version.
pub(crate) async fn states_json(view: &Arc<TargetView>) -> Result<CachedJson, ControlError> {
    let held = view.clone();
    let body = view
        .states_body
        .get_or_try_init(|| serialized(move || serde_json::to_vec(&state_record(&held))))
        .await?
        .clone();
    Ok(CachedJson { etag: None, body })
}

async fn serialized(
    write: impl FnOnce() -> serde_json::Result<Vec<u8>> + Send + 'static,
) -> Result<Bytes, ControlError> {
    tokio::task::spawn_blocking(move || {
        write()
            .map(Bytes::from)
            .map_err(|error| ControlError::Invalid(error.to_string()))
    })
    .await?
}

fn state_record(view: &TargetView) -> NodeStatesRecord {
    NodeStatesRecord {
        snapshot_id: view.catalog.snapshot_id,
        version: view.version,
        states: (*view.states).clone(),
        changed_build: view.changed_build.as_deref().cloned(),
    }
}
