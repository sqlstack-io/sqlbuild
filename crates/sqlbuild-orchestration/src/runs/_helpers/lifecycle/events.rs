//! Lifecycle facts share Recall's invocation sequence and transaction.

use serde_json::Value;
use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::append_control_event::append_control_event;
use crate::store::models::{ControlEvent, ControlTransaction};

pub(crate) async fn append_run_event(
    transaction: ControlTransaction,
    (run_id, event_id): (Uuid, Uuid),
    kind: &str,
    payload: Value,
) -> Result<(ControlTransaction, i64), ControlError> {
    let event = ControlEvent {
        event_id,
        invocation_id: format!("scheduler/{run_id}"),
        event_type: kind,
        producer: "scheduler",
        occurred_at: transaction.now,
        run_id: Some(run_id),
        payload,
    };
    let (transaction, outcome) = append_control_event(transaction, event).await?;
    if outcome.conflict.is_some() {
        return Err(ControlError::Invalid(
            "run event conflicts with a durable fact".into(),
        ));
    }
    let cursor = outcome
        .storage_id
        .ok_or_else(|| ControlError::Invalid("run event has no storage cursor".into()))?;
    Ok((transaction, cursor))
}
