//! Attached checks are immutable observations, not additions to the frozen execution selection.

use crate::runs::models::{NodeOutcome, RunChange, RunChangeRequest};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn check_execution_history(
    transaction: ControlTransaction,
    request: &RunChangeRequest,
) -> Result<ControlTransaction, ControlError> {
    let (node_key, outcome) = match &request.change {
        RunChange::CheckStarted { node_key } => (node_key, None),
        RunChange::CheckFinished { node_key, outcome } => (node_key, Some(*outcome)),
        _ => return Err(ControlError::Invalid("check fact required".into())),
    };
    let (transaction, nodes) = query(transaction,
        "SELECT n.node_key FROM sqb_runs r JOIN sqb_snapshot_nodes n ON n.snapshot_id=r.snapshot_id \
         WHERE r.run_id=$1 AND n.node_key=$2 AND n.node_kind='check' LIMIT 1",
        vec![Parameter::Id(request.run_id), Parameter::Text(node_key.clone())]).await?;
    if !node_key.starts_with("check:") || nodes.is_empty() {
        return Err(ControlError::Invalid(
            "check is outside the frozen snapshot".into(),
        ));
    }
    let (transaction, previous) = query(
        transaction,
        "SELECT event_type FROM recall_events WHERE invocation_id=$1 AND producer='scheduler' \
         AND run_id=$2 AND event_type IN ('run_check_started','run_check_finished') \
         AND (payload -> 'change' -> 'change' ->> 'node_key')=$3 \
         AND (payload -> 'change' ->> 'claim_id')=$4 AND event_id<>$5 \
         ORDER BY storage_id DESC LIMIT 1",
        vec![
            Parameter::Text(format!("scheduler/{}", request.run_id)),
            Parameter::Text(request.run_id.to_string()),
            Parameter::Text(node_key.clone()),
            Parameter::OptionalText(request.claim_id.map(|id| id.to_string())),
            Parameter::Text(request.event_id.to_string()),
        ],
    )
    .await?;
    let previous = previous.first().and_then(|row| row["event_type"].as_str());
    let valid = match outcome {
        None => previous.is_none(),
        Some(NodeOutcome::Skipped) => previous.is_none() || previous == Some("run_check_started"),
        Some(_) => previous == Some("run_check_started"),
    };
    if !valid {
        return Err(ControlError::Invalid(
            "check fact is inconsistent with its execution history".into(),
        ));
    }
    Ok(transaction)
}
