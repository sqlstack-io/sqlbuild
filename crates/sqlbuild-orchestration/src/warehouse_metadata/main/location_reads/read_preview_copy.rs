//! A plan preview plans from the mirror, so the copy carries only what its facade may answer.

use uuid::Uuid;

use crate::platform::main::location_health::location_health;
use crate::store::errors::ControlError;
use crate::store::main::commit::commit;
use crate::store::models::ControlStore;
use crate::warehouse_metadata::_helpers::ingestion::begin_ingestion;
use crate::warehouse_metadata::_helpers::mirror_pages::{
    preview_fingerprints, preview_latest_runs, preview_observations, preview_relations, synced_at,
};
use crate::warehouse_metadata::constants::PREVIEW_REQUIRED_TABLES;
use crate::warehouse_metadata::models::{PreviewCopy, PreviewCopyRead};

pub async fn read_preview_copy(
    store: &ControlStore,
    scope: (Uuid, &str),
    request: &PreviewCopyRead,
) -> Result<PreviewCopy, ControlError> {
    let transaction =
        begin_ingestion(store, scope, (request.session_id, request.generation)).await?;
    let (transaction, relations) = preview_relations(transaction, scope).await?;
    let (transaction, fingerprints) = preview_fingerprints(transaction, scope).await?;
    let (transaction, latest_runs) = preview_latest_runs(transaction, scope).await?;
    let (transaction, observations) = preview_observations(transaction, scope).await?;
    commit(transaction).await?;
    let health = location_health(store, (scope.0, Some(scope.1))).await?;
    Ok(PreviewCopy {
        synced_at: synced_at(&health, &PREVIEW_REQUIRED_TABLES),
        relations,
        fingerprints,
        latest_runs,
        observations,
    })
}
