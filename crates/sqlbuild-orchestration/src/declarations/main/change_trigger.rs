//! Apply an operator fact and its projection in one transaction, with retry-safe request identity.

use serde_json::json;

use crate::credentials::main::append_audit::append_audit;
use crate::credentials::models::AuditContext;
use crate::declarations::_helpers::trigger_state::trigger_timing;
use crate::declarations::models::{
    EditableTriggerState, TriggerChange, TriggerChangeRequest, TriggerStatus,
};
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::decode_row::decode_row;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// Authorization belongs to the API boundary; `actor` is the authenticated principal, not client input.
pub async fn change_trigger(
    store: &ControlStore,
    context: &AuditContext,
    request: &TriggerChangeRequest,
) -> Result<bool, ControlError> {
    if request.actor.trim().is_empty() || request.actor != context.actor {
        return Err(ControlError::Invalid(
            "trigger changes require the authenticated audit actor".into(),
        ));
    }
    let body =
        serde_json::to_value(request).map_err(|error| ControlError::Invalid(error.to_string()))?;
    let transaction = begin(store, TransactionMode::Write).await?;
    let (transaction, previous) = query(
        transaction,
        "SELECT request_json FROM sqb_trigger_state_events WHERE event_id=$1",
        vec![Parameter::Id(request.event_id)],
    )
    .await?;
    if let Some(previous) = previous.first() {
        if previous.get("request_json") != Some(&body) {
            return Err(ControlError::Invalid(
                "trigger change event ID already has different content".into(),
            ));
        }
        commit(transaction).await?;
        return Ok(false);
    }
    let (transaction, rows) = query(
        transaction,
        "SELECT t.definition_json, t.snapshot_id, s.started_at, s.last_tick_for, s.status_override \
         FROM sqb_triggers t JOIN sqb_trigger_state s ON s.trigger_id=t.trigger_id \
         WHERE t.trigger_id=$1 AND t.removed_in_snapshot_id IS NULL",
        vec![Parameter::Id(request.trigger_id)],
    )
    .await?;
    let mut state: EditableTriggerState =
        decode_row(rows.into_iter().next().ok_or_else(|| {
            ControlError::Invalid("trigger does not exist in the current snapshot".into())
        })?)?;
    let before = json!({
        "status_override": state.status_override,
        "last_tick_for": state.last_tick_for,
    });
    match request.change {
        TriggerChange::Enable => state.status_override = Some(TriggerStatus::Running),
        TriggerChange::Pause => state.status_override = Some(TriggerStatus::Paused),
        TriggerChange::ResetDefault => state.status_override = None,
        TriggerChange::SetCursor { time } => state.last_tick_for = Some(time),
        TriggerChange::ResetCursor => state.last_tick_for = None,
    }
    let (status, next) = trigger_timing(
        &state.definition_json,
        state.started_at,
        state.last_tick_for,
        state.status_override,
    )?;
    let now = transaction.now;
    let (transaction, _) = query(transaction,
        "INSERT INTO sqb_trigger_state_events (event_id, trigger_id, occurred_at, snapshot_id, request_json) VALUES ($1,$2,$3,$4,$5)",
        vec![Parameter::Id(request.event_id), Parameter::Id(request.trigger_id), Parameter::Time(now), Parameter::Id(state.snapshot_id), Parameter::Json(body)]).await?;
    let (transaction, _) = query(transaction,
        "UPDATE sqb_trigger_state SET status=$1, status_override=$2, last_tick_for=$3, next_due_at=$4, updated_at=$5 WHERE trigger_id=$6",
        vec![Parameter::Text(status.as_str().into()), Parameter::OptionalText(state.status_override.map(|value| value.as_str().into())), Parameter::OptionalTime(state.last_tick_for),
            Parameter::OptionalTime(next), Parameter::Time(now), Parameter::Id(request.trigger_id)]).await?;
    commit(
        append_audit(
            transaction,
            context,
            (request.event_id, "trigger.changed"),
            (
                json!({"trigger_id": request.trigger_id, "snapshot_id": state.snapshot_id}),
                before,
                json!({
                    "status_override": state.status_override,
                    "last_tick_for": state.last_tick_for,
                    "reason": request.reason,
                }),
            ),
        )
        .await?,
    )
    .await?;
    Ok(true)
}
