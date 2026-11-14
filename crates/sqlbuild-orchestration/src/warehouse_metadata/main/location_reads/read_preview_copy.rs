//! A plan preview plans from the mirror, so the copy carries only what its facade may answer.

use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::commit::commit;
use crate::store::models::ControlStore;
use crate::warehouse_metadata::_helpers::ingestion::begin_ingestion;
use crate::warehouse_metadata::_helpers::mirror_pages::{
    preview_consumption, preview_fingerprints, preview_latest_runs, preview_observations,
    preview_relations, preview_synced_at,
};
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
    let (transaction, consumption) = if request.consumption {
        preview_consumption(transaction, scope).await?
    } else {
        (transaction, Vec::new())
    };
    let (transaction, synced_at) = preview_synced_at(transaction, scope).await?;
    commit(transaction).await?;
    Ok(PreviewCopy {
        synced_at,
        relations,
        fingerprints,
        latest_runs,
        observations,
        consumption,
    })
}
