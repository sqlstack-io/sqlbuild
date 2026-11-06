//! Attached checks are recorded beside nodes, but never join the frozen execution selection.

use crate::runs::constants::NODE_RUNNING;
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
        "SELECT status FROM sqb_run_nodes WHERE run_id=$1 AND node_key=$2 AND kind='check'",
        vec![
            Parameter::Id(request.run_id),
            Parameter::Text(node_key.clone()),
        ],
    )
    .await?;
    let previous = previous.first().and_then(|row| row["status"].as_str());
    let valid = match outcome {
        None => previous.is_none(),
        Some(NodeOutcome::Skipped) => previous.is_none() || previous == Some(NODE_RUNNING),
        Some(_) => previous == Some(NODE_RUNNING),
    };
    if !valid {
        return Err(ControlError::Invalid(
            "check fact is inconsistent with its execution history".into(),
        ));
    }
    let now = transaction.now;
    let status = outcome.map_or(NODE_RUNNING, |outcome| outcome.as_str());
    let (transaction, _) = query(
        transaction,
        "INSERT INTO sqb_run_nodes (run_id, node_key, status, writes, started_at, ended_at, kind) \
         VALUES ($1,$2,$3,FALSE,$4,$5,'check') ON CONFLICT (run_id, node_key) \
         DO UPDATE SET status=excluded.status, ended_at=excluded.ended_at",
        vec![
            Parameter::Id(request.run_id),
            Parameter::Text(node_key.clone()),
            Parameter::Text(status.into()),
            Parameter::OptionalTime(outcome.is_none().then_some(now)),
            Parameter::OptionalTime(outcome.is_some().then_some(now)),
        ],
    )
    .await?;
    Ok(transaction)
}
