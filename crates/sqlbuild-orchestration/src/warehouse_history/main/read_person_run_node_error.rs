//! Error text can quote warehouse data, so it needs view.evidence on that node; the read is audited like evidence.

use serde_json::{Value, json};
use uuid::Uuid;

use crate::authorization::main::begin_authorized::begin_authorized;
use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::models::{Action, AuditContext, DefaultRole, PermissionScope};
use crate::runs::models::{RunChange, RunChangeRequest, RunNodeFailure};
use crate::snapshots::main::read_snapshot_nodes::read_snapshot_nodes;
use crate::store::errors::ControlError;
use crate::store::main::{commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter};

/// `None` when the node is not in that run of this location and target; refusal comes first either way.
pub async fn read_person_run_node_error(
    store: &ControlStore,
    credentials: (&AuditContext, Uuid),
    (location_id, target): (Uuid, &str),
    (run_id, node_key): (Uuid, &str),
) -> Result<Option<RunNodeFailure>, ControlError> {
    let scope = PermissionScope {
        action: Action::ViewEvidence,
        location_id: Some(location_id),
        target: Some(target.into()),
        nodes: [node_key.into()].into(),
    };
    let (transaction, context) =
        begin_authorized(store, credentials, &scope, DefaultRole::None).await?;
    let (mut transaction, rows) = query(
        transaction,
        "SELECT r.snapshot_id, n.error_code, e.payload FROM sqb_runs r \
         JOIN sqb_run_nodes n ON n.run_id=r.run_id AND n.kind='node' AND n.node_key=$4 \
         LEFT JOIN recall_events e ON e.storage_id=n.finished_storage_id \
         WHERE r.run_id=$1 AND r.location_id=$2 AND r.target=$3",
        vec![
            Parameter::Id(run_id),
            Parameter::Id(location_id),
            Parameter::Text(target.into()),
            Parameter::Text(node_key.into()),
        ],
    )
    .await?;
    let mut result = None;
    if let Some(row) = rows.into_iter().next() {
        let message = match &row["payload"] {
            Value::Null => None,
            payload => match serde_json::from_value::<RunChangeRequest>(payload["change"].clone())
                .map_err(|error| ControlError::Invalid(error.to_string()))?
                .change
            {
                RunChange::NodeFinished { error_message, .. } => error_message,
                _ => None,
            },
        };
        let snapshot_id: Uuid = serde_json::from_value(row["snapshot_id"].clone())
            .map_err(|error| ControlError::Invalid(error.to_string()))?;
        let nodes;
        (transaction, nodes) =
            read_snapshot_nodes(transaction, snapshot_id, Some(&scope.nodes)).await?;
        result = Some(RunNodeFailure {
            node_key: node_key.into(),
            error_code: row["error_code"].as_str().map(str::to_owned),
            message,
            path: nodes
                .get(node_key)
                .and_then(|node| node.definition.get("path"))
                .and_then(Value::as_str)
                .map(|path| path.replace('\\', "/")),
        });
    }
    let transaction = append_audit(
        transaction,
        &context,
        (Uuid::now_v7(), "error.read"),
        (
            json!(scope),
            json!(null),
            json!({"run_id": run_id, "found": result.is_some()}),
        ),
    )
    .await?;
    commit(transaction).await?;
    Ok(result)
}
