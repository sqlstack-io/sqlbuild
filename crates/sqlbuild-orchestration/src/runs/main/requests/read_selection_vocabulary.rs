//! The values each selector type takes in the current publication, for a person who may view it.

use crate::runs::_helpers::requests::prepared_calls::vocabulary_off_worker;
use uuid::Uuid;

use crate::credentials::models::{AuditContext, DefaultRole};
use crate::runs::_helpers::lifecycle::publications::viewed_publication;
use crate::runs::models::SelectionVocabulary;
use crate::runs::types::CommandPlanner;
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

/// Tags, folders, jobs, sources, seeds and kinds with their node counts; empty before the first publication.
pub async fn read_selection_vocabulary(
    store: &ControlStore,
    (planner, credentials): (&dyn CommandPlanner, (&AuditContext, Uuid)),
    (location_id, target): (Uuid, &str),
    default_role: DefaultRole,
) -> Result<SelectionVocabulary, ControlError> {
    let current = viewed_publication(
        store,
        (planner, credentials),
        (location_id, target),
        default_role,
    )
    .await?;
    let Some((snapshot, publication)) = current else {
        return Ok(SelectionVocabulary::default());
    };
    Ok(SelectionVocabulary {
        snapshot_id: Some(snapshot.snapshot_id),
        ..vocabulary_off_worker(&publication.prepared).await?
    })
}
