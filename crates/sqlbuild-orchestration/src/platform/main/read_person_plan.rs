//! A plan is read only by the person who asked for it, with view still held on every current node.

use std::collections::BTreeSet;

use uuid::Uuid;

use crate::authorization::main::authorize_listing::authorize_listing;
use crate::authorization::main::decide_person_scopes::decide_person_scopes;
use crate::credentials::models::{Action, AuditContext, DefaultRole, PermissionScope};
use crate::platform::main::read_location_work::read_location_work;
use crate::platform::models::{LocationWork, LocationWorkOutcome, LocationWorkRecord};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, ControlTransaction, Parameter, TransactionMode};

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
    let (transaction, stored) = read_location_work(transaction, request_id).await?;
    let Some(mut record) = stored.map(|stored| stored.record).filter(|record| {
        record.requested_by == context.actor
            && record.request_json.location_id == location_id
            && record.request_json.target == target
            && matches!(record.request_json.work, LocationWork::Plan { .. })
    }) else {
        commit(transaction).await?;
        return Ok(None);
    };
    let (transaction, sql) = may_read_sql(transaction, principal_id, (location_id, target)).await?;
    commit(transaction).await?;
    if !sql {
        without_identity_diffs(&mut record);
    }
    Ok(Some(record))
}

/// Whether the person holds `view.sql` on every current node, decided without recording a denial.
async fn may_read_sql(
    transaction: ControlTransaction,
    principal_id: Uuid,
    (location_id, target): (Uuid, &str),
) -> Result<(ControlTransaction, bool), ControlError> {
    let (transaction, current) = query(transaction,
        "SELECT n.node_key FROM sqb_snapshot_nodes n JOIN sqb_current_snapshots c ON c.snapshot_id=n.snapshot_id \
         WHERE c.location_id=$1 AND c.target=$2",
        vec![Parameter::Id(location_id), Parameter::Text(target.into())]).await?;
    let scope = PermissionScope {
        action: Action::ViewSql,
        location_id: Some(location_id),
        target: Some(target.into()),
        nodes: current
            .iter()
            .filter_map(|row| row["node_key"].as_str().map(str::to_owned))
            .collect::<BTreeSet<String>>(),
    };
    let (transaction, refusals) =
        decide_person_scopes(transaction, principal_id, (target, &[scope])).await?;
    Ok((transaction, refusals.as_deref() == Some(&[None])))
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
