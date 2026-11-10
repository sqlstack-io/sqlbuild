//! A plan is read only by the person who asked for it, with view still held on every current node.

use uuid::Uuid;

use crate::authorization::main::authorize_listing::authorize_listing;
use crate::credentials::models::{Action, AuditContext, DefaultRole};
use crate::platform::main::_read_target_work::read_target_work;
use crate::platform::models::{LocationWork, LocationWorkOutcome, LocationWorkRecord};
use crate::snapshots::main::holds_on_current_nodes::holds_on_current_nodes;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

/// Python source and dependency diffs stay out of the plan unless the reader may also read SQL on every current node.
pub async fn read_person_plan(
    store: &ControlStore,
    (context, principal_id): (&AuditContext, Uuid),
    (location_id, target, request_id): (Uuid, &str, Uuid),
) -> Result<Option<LocationWorkRecord>, ControlError> {
    let (transaction, context) = authorize_listing(
        begin(store, TransactionMode::Write).await?,
        (context, principal_id),
        (location_id, target),
        DefaultRole::None,
    )
    .await?;
    let (transaction, record) =
        read_target_work(transaction, (location_id, target, request_id)).await?;
    let Some(mut record) = record.filter(|record| {
        record.requested_by == context.actor
            && matches!(record.request_json.work, LocationWork::Plan { .. })
    }) else {
        commit(transaction).await?;
        return Ok(None);
    };
    let (transaction, sql) = holds_on_current_nodes(
        transaction,
        principal_id,
        (Action::ViewSql, location_id, target),
    )
    .await?;
    commit(transaction).await?;
    if !sql {
        without_identity_diffs(&mut record);
    }
    Ok(Some(record))
}

fn without_identity_diffs(record: &mut LocationWorkRecord) {
    let Some(LocationWorkOutcome::Plan { document }) = record
        .result_json
        .as_mut()
        .map(|result| &mut result.outcome)
    else {
        return;
    };
    let nodes = document
        .get_mut("python_nodes")
        .and_then(serde_json::Value::as_array_mut);
    for node in nodes.into_iter().flatten() {
        if let Some(node) = node.as_object_mut() {
            node.remove("identity_diff");
        }
    }
}
