//! Publish a lifecycle fact, its projection and terminal cleanup in the caller's transaction.

use serde_json::json;

use crate::runs::_helpers::events::append_run_event;
use crate::runs::_helpers::projection::project_change;
use crate::runs::models::{ChangingRun, RunChange, RunChangeRequest, RunOutcome};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn publish_change(
    transaction: ControlTransaction,
    request: &RunChangeRequest,
    state: &ChangingRun,
) -> Result<ControlTransaction, ControlError> {
    let (transaction, mut cursor) = append_run_event(
        transaction,
        (request.run_id, request.event_id),
        request.change.event_type(),
        json!({"change": request}),
    )
    .await?;
    let mut transaction = project_change(transaction, request, state).await?;
    if matches!(request.change, RunChange::CancelRequested { .. })
        && state.worker_instance.is_none()
    {
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
