//! Node lifecycle projections adjust bounded counters instead of rescanning nodes per event.

use std::collections::BTreeMap;

use serde_json::json;

use crate::runs::constants::{NODE_PENDING, NODE_RUNNING};
use crate::runs::models::{RunChange, RunChangeRequest};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn project_node(
    transaction: ControlTransaction,
    request: &RunChangeRequest,
    counts: &BTreeMap<String, i64>,
) -> Result<ControlTransaction, ControlError> {
    let (node, status, rows, error, skipped) = match &request.change {
        RunChange::NodeStarted { node_key } => (node_key, NODE_RUNNING, None, None, None),
        RunChange::NodeFinished {
            node_key,
            outcome,
            rows_affected,
            error_code,
            skipped_because_of,
        } => (
            node_key,
            outcome.as_str(),
            *rows_affected,
            error_code.clone(),
            skipped_because_of.clone(),
        ),
        _ => {
            return Err(ControlError::Invalid(
                "node projection requires a node event".into(),
            ));
        }
    };
    let now = transaction.now;
    let (transaction, nodes) = query(
        transaction,
        "SELECT status FROM sqb_run_nodes WHERE run_id=$1 AND node_key=$2 AND attempt=1",
        vec![Parameter::Id(request.run_id), Parameter::Text(node.clone())],
    )
    .await?;
    let previous = nodes
        .first()
        .and_then(|row| row.get("status"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| ControlError::Invalid("node is outside the frozen run selection".into()))?;
    if previous != NODE_PENDING && (previous != NODE_RUNNING || status == NODE_RUNNING) {
        return Err(ControlError::Invalid(
            "node change is inconsistent with its current outcome".into(),
        ));
    }
    let mut counts = counts.clone();
    let old = counts
        .get_mut(previous)
        .ok_or_else(|| ControlError::Invalid("node counter is missing".into()))?;
    *old -= 1;
    if *old < 0 {
        return Err(ControlError::Invalid("node counter is negative".into()));
    }
    if *old == 0 {
        counts.remove(previous);
    }
    *counts.entry(status.into()).or_default() += 1;
    let (transaction, _) = query(transaction,
        "UPDATE sqb_run_nodes SET status=$1, started_at=CASE WHEN $1='running' THEN $2 ELSE started_at END, \
         ended_at=$3, rows_affected=$4, error_code=$5, skipped_because_of=$6 WHERE run_id=$7 AND node_key=$8 AND attempt=1",
        vec![Parameter::Text(status.into()), Parameter::Time(now), Parameter::OptionalTime((status != NODE_RUNNING).then_some(now)),
            rows.map_or(Parameter::Null(sqlbuild_recall::schema::models::ColumnType::Integer), Parameter::Integer),
            Parameter::OptionalText(error), Parameter::OptionalText(skipped), Parameter::Id(request.run_id), Parameter::Text(node.clone())]).await?;
    let (transaction, _) = query(
        transaction,
        "UPDATE sqb_runs SET node_counts_json=$1 WHERE run_id=$2",
        vec![
            Parameter::Json(json!(counts)),
            Parameter::Id(request.run_id),
        ],
    )
    .await?;
    Ok(transaction)
}
