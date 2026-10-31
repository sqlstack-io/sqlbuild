//! Lifecycle projections and claim cleanup are part of the fact's transaction, never a later task.

use std::collections::BTreeMap;

use serde_json::json;

use crate::runs::_helpers::lifecycle::nodes::project_node;
use crate::runs::models::{ChangingRun, RunChange, RunChangeRequest, RunOutcome};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn project_change(
    mut transaction: ControlTransaction,
    request: &RunChangeRequest,
    state: &ChangingRun,
) -> Result<ControlTransaction, ControlError> {
    let now = transaction.now;
    match &request.change {
        RunChange::NodeStarted { .. } | RunChange::NodeFinished { .. } => {
            return project_node(transaction, request, &state.node_counts_json).await;
        }
        RunChange::Launched { container_id } => {
            (transaction, _) = query(transaction,
                "UPDATE sqb_runs SET status=CASE WHEN status='queued' THEN 'launched' ELSE status END, launched_at=COALESCE(launched_at,$1) WHERE run_id=$2",
                vec![Parameter::Time(now), Parameter::Id(request.run_id)]).await?;
            (transaction, _) = query(
                transaction,
                "UPDATE sqb_run_liveness SET container_id=$1 WHERE run_id=$2",
                vec![
                    Parameter::OptionalText(container_id.clone()),
                    Parameter::Id(request.run_id),
                ],
            )
            .await?;
        }
        RunChange::Started => {
            (transaction, _) = query(transaction,
                "UPDATE sqb_runs SET status='running', launched_at=COALESCE(launched_at,$1), started_at=$1 WHERE run_id=$2",
                vec![Parameter::Time(now), Parameter::Id(request.run_id)]).await?;
        }
        RunChange::Unresponsive | RunChange::Recovered => {
            let status = if request.change == RunChange::Unresponsive {
                "unresponsive"
            } else {
                "running"
            };
            (transaction, _) = query(
                transaction,
                "UPDATE sqb_runs SET status=$1 WHERE run_id=$2",
                vec![
                    Parameter::Text(status.into()),
                    Parameter::Id(request.run_id),
                ],
            )
            .await?;
        }
        RunChange::CancelRequested { .. } => {
            (transaction, _) = query(
                transaction,
                "UPDATE sqb_runs SET cancel_requested_at=$1 WHERE run_id=$2",
                vec![Parameter::Time(now), Parameter::Id(request.run_id)],
            )
            .await?;
        }
        RunChange::Finished {
            outcome, reason, ..
        } => {
            let cancelled = *outcome == RunOutcome::Cancelled;
            (transaction, _) = query(transaction,
                "UPDATE sqb_run_nodes SET status=CASE WHEN status='running' THEN $1 ELSE $2 END, ended_at=$3 \
                 WHERE run_id=$4 AND status IN ('pending','running')",
                vec![Parameter::Text(if cancelled { "cancelled" } else { "failed" }.into()),
                    Parameter::Text(if cancelled { "cancelled" } else { "skipped" }.into()), Parameter::Time(now), Parameter::Id(request.run_id)]).await?;
            let counts;
            (transaction, counts) = query(transaction,
                "SELECT status, COUNT(*) AS count FROM sqb_run_nodes WHERE run_id=$1 GROUP BY status",
                vec![Parameter::Id(request.run_id)]).await?;
            let counts: BTreeMap<String, i64> = counts
                .into_iter()
                .filter_map(|row| {
                    Some((
                        row.get("status")?.as_str()?.into(),
                        row.get("count")?.as_i64()?,
                    ))
                })
                .collect();
            (transaction, _) = query(
                transaction,
                "UPDATE sqb_runs SET status=$1, status_reason=$2, ended_at=$3, node_counts_json=$5 WHERE run_id=$4",
                vec![
                    Parameter::Text(outcome.as_str().into()),
                    Parameter::Text(reason.clone()),
                    Parameter::Time(now),
                    Parameter::Id(request.run_id),
                    Parameter::Json(json!(counts)),
                ],
            )
            .await?;
            for table in [
                "sqb_write_claims",
                "sqb_concurrency_slots",
                "sqb_run_liveness",
            ] {
                (transaction, _) = query(
                    transaction,
                    &format!("DELETE FROM {table} WHERE run_id=$1"),
                    vec![Parameter::Id(request.run_id)],
                )
                .await?;
            }
        }
    }
    Ok(transaction)
}
