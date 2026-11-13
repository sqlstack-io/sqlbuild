//! A failed or partial run becomes one platform event naming its job, scope and first failed nodes.

use std::collections::BTreeSet;

use serde_json::{Value, json};

use crate::platform::constants::RUN_FAILURE_NODE_LIMIT;
use crate::platform::models::PlatformEvent;
use crate::runs::_helpers::lifecycle::error_text::finished_error_message;
use crate::runs::models::{RunChangeRequest, RunNodeFailure, RunOutcome};
use crate::snapshots::main::read_snapshot_nodes::read_snapshot_nodes;
use crate::store::errors::ControlError;
use crate::store::main::append_control_event::append_control_event;
use crate::store::main::query::query;
use crate::store::models::{ControlEvent, ControlTransaction, Parameter};

/// Runs after the terminal projection, so nodes left running already read as failed.
pub(crate) async fn publish_run_failure(
    transaction: ControlTransaction,
    request: &RunChangeRequest,
    outcome: RunOutcome,
) -> Result<ControlTransaction, ControlError> {
    let invalid = |error: serde_json::Error| ControlError::Invalid(error.to_string());
    let (transaction, runs) = query(
        transaction,
        "SELECT kind, job_key, location_id, target, snapshot_id FROM sqb_runs WHERE run_id=$1",
        vec![Parameter::Id(request.run_id)],
    )
    .await?;
    let run = runs
        .into_iter()
        .next()
        .ok_or_else(|| ControlError::Invalid("finished run is missing".into()))?;
    let (transaction, counted) = query(
        transaction,
        "SELECT COUNT(*) AS count FROM sqb_run_nodes WHERE run_id=$1 AND kind='node' AND status='failed'",
        vec![Parameter::Id(request.run_id)],
    )
    .await?;
    let (transaction, rows) = query(
        transaction,
        "SELECT n.node_key, n.error_code, e.payload FROM sqb_run_nodes n \
         LEFT JOIN recall_events e ON e.storage_id=n.finished_storage_id \
         WHERE n.run_id=$1 AND n.kind='node' AND n.status='failed' ORDER BY n.node_key LIMIT $2",
        vec![
            Parameter::Id(request.run_id),
            Parameter::Integer(RUN_FAILURE_NODE_LIMIT),
        ],
    )
    .await?;
    let keys: BTreeSet<String> = rows
        .iter()
        .filter_map(|row| row["node_key"].as_str().map(str::to_owned))
        .collect();
    let snapshot_id = serde_json::from_value(run["snapshot_id"].clone()).map_err(invalid)?;
    let (mut transaction, nodes) =
        read_snapshot_nodes(transaction, snapshot_id, Some(&keys)).await?;
    let mut failed_nodes: Vec<RunNodeFailure> = Vec::new();
    for row in rows {
        let node_key = row["node_key"].as_str().unwrap_or_default().to_owned();
        failed_nodes.push(RunNodeFailure {
            error_code: row["error_code"].as_str().map(str::to_owned),
            message: finished_error_message(&row["payload"])?,
            path: nodes
                .get(&node_key)
                .and_then(|node| node.definition.get("path"))
                .and_then(Value::as_str)
                .map(|path| path.replace('\\', "/")),
            node_key,
        });
    }
    let platform = PlatformEvent::RunFailed {
        run_id: request.run_id,
        run_kind: serde_json::from_value(run["kind"].clone()).map_err(invalid)?,
        job_key: run["job_key"].as_str().map(str::to_owned),
        location_id: serde_json::from_value(run["location_id"].clone()).map_err(invalid)?,
        target: run["target"].as_str().unwrap_or_default().to_owned(),
        outcome,
        failed_nodes,
        failed_node_count: counted
            .first()
            .and_then(|row| row["count"].as_i64())
            .unwrap_or_default(),
    };
    let occurred_at = transaction.now;
    let appended;
    (transaction, appended) = append_control_event(
        transaction,
        ControlEvent {
            event_id: uuid::Uuid::new_v5(&request.event_id, b"platform-run-failed"),
            invocation_id: format!("platform/{}", request.run_id),
            event_type: platform.event_type(),
            producer: "platform",
            occurred_at,
            run_id: Some(request.run_id),
            payload: json!({"platform": platform}),
        },
    )
    .await?;
    if appended.conflict.is_some() {
        return Err(ControlError::Invalid(
            "run failure conflicts with a durable fact".into(),
        ));
    }
    Ok(transaction)
}
