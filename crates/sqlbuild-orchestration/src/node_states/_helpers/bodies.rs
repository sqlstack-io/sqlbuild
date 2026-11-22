//! Response bodies are serialized once, off the async workers: the node list once per snapshot, which also tags it, the states once per view, and the check list once per change, tagged by its content.

use std::sync::Arc;

use axum::body::Bytes;
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;
use uuid::Uuid;

use crate::node_states::constants::CHECKS_TAG_DIGEST_CHARS;
use crate::node_states::models::{CachedJson, NodeStatesRecord, TargetView};
use crate::store::errors::ControlError;
use crate::warehouse_history::models::CheckResult;

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

/// The check list, tagged by its snapshot and a digest of the body, so the same results give the same tag on any server.
pub(crate) async fn checks_json(
    snapshot_id: Option<Uuid>,
    checks: Vec<CheckResult>,
) -> Result<CachedJson, ControlError> {
    let (body, digest) = tokio::task::spawn_blocking(move || {
        let body = serde_json::to_vec(&checks)
            .map_err(|error| ControlError::Invalid(error.to_string()))?;
        let digest = sha256_hex(&body);
        Ok::<_, ControlError>((Bytes::from(body), digest))
    })
    .await??;
    let digest = &digest[..CHECKS_TAG_DIGEST_CHARS];
    Ok(CachedJson {
        etag: Some(match snapshot_id {
            Some(snapshot_id) => format!("\"{snapshot_id}:{digest}\""),
            None => format!("\"{digest}\""),
        }),
        body,
    })
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
