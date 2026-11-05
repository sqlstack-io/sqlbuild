//! Publish facts atomically; launch observations never project state or release physical ownership.

use serde_json::json;

use crate::platform::models::PlatformEvent;
use crate::runs::_helpers::lifecycle::events::append_run_event;
use crate::runs::_helpers::lifecycle::projection::project_change;
use crate::runs::models::{ChangingRun, RunChange, RunChangeRequest, RunOutcome};
use crate::store::errors::ControlError;
use crate::store::main::append_control_event::append_control_event;
use crate::store::main::query::query;
use crate::store::models::{ControlEvent, ControlTransaction, Parameter};

pub(crate) async fn publish_change(
    transaction: ControlTransaction,
    request: &RunChangeRequest,
    state: &ChangingRun,
) -> Result<ControlTransaction, ControlError> {
    let (mut transaction, mut cursor) = append_run_event(
        transaction,
        (request.run_id, request.event_id),
        request.change.event_type(),
        json!({"change": request}),
    )
    .await?;
    if let (RunChange::LaunchFailed { code }, Some(claim_id)) = (&request.change, request.claim_id)
    {
        let platform = PlatformEvent::LaunchFailed {
            run_id: request.run_id,
            claim_id,
            code: code.clone(),
        };
        let occurred_at = transaction.now;
        let outcome;
        (transaction, outcome) = append_control_event(
            transaction,
            ControlEvent {
                event_id: uuid::Uuid::new_v5(&request.event_id, b"platform-launch-failed"),
                invocation_id: format!("platform/{}", request.run_id),
                event_type: platform.event_type(),
                producer: "platform",
                occurred_at,
                run_id: Some(request.run_id),
                payload: json!({"platform": platform}),
            },
        )
        .await?;
        if outcome.conflict.is_some() {
            return Err(ControlError::Invalid(
                "launch failure conflicts with a durable fact".into(),
            ));
        }
        return Ok(transaction);
    }
    let mut transaction = project_change(transaction, request, state).await?;
    if let Some((node_key, _)) = request.change.finished_node() {
        (transaction, _) = query(
            transaction,
            "UPDATE sqb_run_nodes SET finished_storage_id=$1 WHERE run_id=$2 AND node_key=$3 AND attempt=1",
            vec![
                Parameter::Integer(cursor),
                Parameter::Id(request.run_id),
                Parameter::Text(node_key.into()),
            ],
        )
        .await?;
    }
    let mut cancel_unstarted = matches!(request.change, RunChange::CancelRequested { .. })
        && state.worker_instance.is_none();
    if cancel_unstarted {
        let rows;
        (transaction, rows) = query(
            transaction,
            "SELECT kind FROM sqb_runs WHERE run_id=$1",
            vec![Parameter::Id(request.run_id)],
        )
        .await?;
        cancel_unstarted = rows.first().and_then(|row| row["kind"].as_str()) != Some("group");
    }
    if cancel_unstarted {
        let terminal = RunChangeRequest {
            event_id: uuid::Uuid::new_v5(&request.event_id, b"cancelled-before-launch"),
            run_id: request.run_id,
            claim_id: None,
            change: RunChange::Finished {
                outcome: RunOutcome::Cancelled,
                reason: "cancelled before launch".into(),
                exit_code: None,
            },
        };
        (transaction, cursor) = append_run_event(
            transaction,
            (terminal.run_id, terminal.event_id),
            terminal.change.event_type(),
            json!({"change": terminal}),
        )
        .await?;
        transaction = project_change(transaction, &terminal, state).await?;
    }
    let (transaction, _) = query(
        transaction,
        "UPDATE sqb_runs SET last_event_storage_id=$1 WHERE run_id=$2",
        vec![Parameter::Integer(cursor), Parameter::Id(request.run_id)],
    )
    .await?;
    Ok(transaction)
}
