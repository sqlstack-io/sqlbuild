//! Publish request facts, queued facts and initial node projections in the caller's transaction.

use serde_json::json;
use uuid::Uuid;

use crate::runs::_helpers::lifecycle::events::append_run_event;
use crate::runs::_helpers::lifecycle::queued::project_queued_run;
use crate::runs::_helpers::requests::job::frozen_group_children;
use crate::runs::models::{RunKind, RunRequest};
use crate::store::errors::ControlError;
use crate::store::models::ControlTransaction;

pub(crate) async fn queue_run(
    transaction: ControlTransaction,
    request: &RunRequest,
) -> Result<ControlTransaction, ControlError> {
    let mut transaction = queue_one(transaction, request).await?;
    if request.kind == RunKind::Group {
        let children;
        (transaction, children) = frozen_group_children(transaction, request).await?;
        for child in children {
            transaction = queue_one(transaction, &child).await?;
        }
    }
    Ok(transaction)
}

async fn queue_one(
    mut transaction: ControlTransaction,
    request: &RunRequest,
) -> Result<ControlTransaction, ControlError> {
    let mut cursor = None;
    for kind in ["run_requested", "run_queued"] {
        let storage_id;
        (transaction, storage_id) = append_run_event(
            transaction,
            (
                request.run_id,
                Uuid::new_v5(&request.run_id, kind.as_bytes()),
            ),
            kind,
            json!({"request": request}),
        )
        .await?;
        cursor = Some(storage_id);
    }
    let cursor =
        cursor.ok_or_else(|| ControlError::Invalid("queued event has no storage cursor".into()))?;
    project_queued_run(transaction, request, cursor).await
}
