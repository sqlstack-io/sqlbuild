//! Failure health is derived from decisive executed facts, never a mutable health flag.

use std::collections::BTreeSet;

use crate::runs::models::NodeOutcome;
use crate::sensors::models::{NodeEventKind, NodeSensorEvent, SensorEvaluationRequest};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn matches_transition(
    transaction: ControlTransaction,
    evaluation: &SensorEvaluationRequest,
    node: &NodeSensorEvent,
    kinds: &BTreeSet<NodeEventKind>,
) -> Result<(ControlTransaction, bool), ControlError> {
    let outcome = node.status;
    if outcome == NodeOutcome::Success && kinds.contains(&NodeEventKind::Success) {
        return Ok((transaction, true));
    }
    let kind = match outcome {
        NodeOutcome::Success => NodeEventKind::Recovered,
        NodeOutcome::Failed => NodeEventKind::Failed,
        _ => return Ok((transaction, false)),
    };
    if !kinds.contains(&kind) {
        return Ok((transaction, false));
    }
    let (transaction, rows) = query(
        transaction,
        "SELECT n.status FROM sqb_run_nodes n JOIN sqb_runs r ON r.run_id=n.run_id \
         WHERE n.node_key=$1 AND n.finished_storage_id<$2 AND n.status IN ('success','failed') \
         AND n.started_at IS NOT NULL AND r.location_id=$3 AND r.target=$4 AND r.kind<>'group' \
         ORDER BY n.finished_storage_id DESC LIMIT 1",
        vec![
            Parameter::Text(node.node_key.clone()),
            Parameter::Integer(node.finished_storage_id),
            Parameter::Id(evaluation.location_id),
            Parameter::Text(evaluation.target.clone()),
        ],
    )
    .await?;
    let failed = rows
        .first()
        .is_some_and(|row| row["status"] == NodeOutcome::Failed.as_str());
    Ok((transaction, (outcome == NodeOutcome::Success) == failed))
}
