//! Lifecycle projections and claim cleanup are part of the fact's transaction, never a later task.

use std::collections::BTreeMap;

use serde_json::json;

use crate::run_output::constants::NODE_LOG_MESSAGE_BYTES;
use crate::runs::_helpers::lifecycle::checks::check_execution_history;
use crate::runs::_helpers::lifecycle::nodes::project_node;
use crate::runs::models::{ChangingRun, RunChange, RunChangeRequest, RunOutcome};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn project_change(
    transaction: ControlTransaction,
    request: &RunChangeRequest,
    state: &ChangingRun,
) -> Result<ControlTransaction, ControlError> {
    if matches!(
        request.change,
        RunChange::CheckStarted { .. } | RunChange::CheckFinished { .. }
    ) {
        return check_execution_history(transaction, request).await;
    }
    let mut transaction = project_run_state(transaction, request, state).await?;
    if let RunChange::Launched { container_id } = &request.change {
        (transaction, _) = query(
            transaction,
            "UPDATE sqb_runs SET container_id=$1 WHERE run_id=$2 AND worker_instance IS NOT NULL",
            vec![
                Parameter::OptionalText(container_id.clone()),
                Parameter::Id(request.run_id),
            ],
        )
        .await?;
    }
    if matches!(request.change, RunChange::Finished { .. }) {
        for statement in [
            "DELETE FROM sqb_write_claims WHERE run_id=$1",
            "DELETE FROM sqb_concurrency_slots WHERE run_id=$1",
            "UPDATE sqb_runs SET worker_instance=NULL, container_id=NULL, claimed_at=NULL, \
             last_heartbeat_at=NULL WHERE run_id=$1",
        ] {
            (transaction, _) =
                query(transaction, statement, vec![Parameter::Id(request.run_id)]).await?;
        }
    }
    Ok(transaction)
}

/// Reconstruct derived status only, without releasing or recreating live physical ownership.
pub(crate) async fn project_run_state(
    mut transaction: ControlTransaction,
    request: &RunChangeRequest,
    state: &ChangingRun,
) -> Result<ControlTransaction, ControlError> {
    let now = transaction.now;
    match &request.change {
        RunChange::LaunchFailed { .. }
        | RunChange::CheckStarted { .. }
        | RunChange::CheckFinished { .. }
        | RunChange::Compiled { .. }
        | RunChange::SinkDeliveries { .. } => {}
        RunChange::NodeStarted { .. } | RunChange::NodeFinished { .. } => {
            return project_node(transaction, request, &state.node_counts_json).await;
        }
        RunChange::NodeLog {
            node_key,
            level,
            message,
        } => {
            if message.len() > NODE_LOG_MESSAGE_BYTES {
                return Err(ControlError::Invalid(
                    "a ctx.log message is longer than its limit".into(),
                ));
            }
            (transaction, _) = query(
                transaction,
                "INSERT INTO sqb_run_node_logs (run_id, event_id, node_key, level, message, logged_at) \
                 VALUES ($1,$2,$3,$4,$5,$6) ON CONFLICT (run_id, event_id) DO NOTHING",
                vec![
                    Parameter::Id(request.run_id),
                    Parameter::Id(request.event_id),
                    Parameter::Text(node_key.clone()),
                    Parameter::Text(level.as_str().into()),
                    Parameter::Text(message.clone()),
                    Parameter::Time(now),
                ],
            )
            .await?;
        }
        RunChange::NodeProgress {
            node_key,
            rows_so_far,
        } => {
            (transaction, _) = query(
                transaction,
                "UPDATE sqb_run_nodes SET rows_so_far=$1 WHERE run_id=$2 AND node_key=$3 \
                 AND kind='node' AND status='running'",
                vec![
                    Parameter::Integer(*rows_so_far),
                    Parameter::Id(request.run_id),
                    Parameter::Text(node_key.clone()),
                ],
            )
            .await?;
        }
        RunChange::AuditFailuresIgnored { count } => {
            (transaction, _) = query(
                transaction,
                "UPDATE sqb_runs SET ignored_audit_failures=$1 WHERE run_id=$2",
                vec![
                    Parameter::Integer(i64::from(*count)),
                    Parameter::Id(request.run_id),
                ],
            )
            .await?;
        }
        RunChange::Launched { .. } => {
            (transaction, _) = query(transaction,
                "UPDATE sqb_runs SET status=CASE WHEN status='queued' THEN 'launched' ELSE status END, launched_at=COALESCE(launched_at,$1) WHERE run_id=$2",
                vec![Parameter::Time(now), Parameter::Id(request.run_id)]).await?;
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
                 WHERE run_id=$4 AND kind='node' AND status IN ('pending','running')",
                vec![Parameter::Text(if cancelled { "cancelled" } else { "failed" }.into()),
                    Parameter::Text(if cancelled { "cancelled" } else { "not_run" }.into()), Parameter::Time(now), Parameter::Id(request.run_id)]).await?;
            let counts;
            (transaction, counts) = query(transaction,
                "SELECT status, COUNT(*) AS count FROM sqb_run_nodes WHERE run_id=$1 AND kind='node' GROUP BY status",
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
        }
    }
    Ok(transaction)
}
