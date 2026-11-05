//! Apply an operator change and its audit record in one transaction, with retry-safe request identity.

use serde_json::json;

use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::models::AuditContext;
use crate::declarations::_helpers::operator_state::apply_operator_change;
use crate::declarations::models::{EditableTriggerState, TriggerChangeRequest};
use crate::store::errors::ControlError;
use crate::store::main::decode_row::decode_row;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn change_trigger_in(
    transaction: ControlTransaction,
    context: &AuditContext,
    request: &TriggerChangeRequest,
) -> Result<(ControlTransaction, bool), ControlError> {
    if request.actor.trim().is_empty() || request.actor != context.actor {
        return Err(ControlError::Invalid(
            "trigger changes require the authenticated audit actor".into(),
        ));
    }
    let body =
        serde_json::to_value(request).map_err(|error| ControlError::Invalid(error.to_string()))?;
    let (transaction, previous) = query(
        transaction,
        "SELECT scope_json FROM sqb_security_audit WHERE event_id=$1 AND action='trigger.changed'",
        vec![Parameter::Id(request.event_id)],
    )
    .await?;
    if let Some(previous) = previous.first() {
        if previous["scope_json"].get("request") != Some(&body) {
            return Err(ControlError::Invalid(
                "trigger change event ID already has different content".into(),
            ));
        }
        return Ok((transaction, false));
    }
    let (transaction, rows) = query(
        transaction,
        "SELECT t.definition_json, t.snapshot_id, s.started_at, s.last_tick_for, s.status_override \
         FROM sqb_triggers t JOIN sqb_trigger_state s ON s.trigger_id=t.trigger_id \
         WHERE t.trigger_id=$1 AND t.removed_in_snapshot_id IS NULL",
        vec![Parameter::Id(request.trigger_id)],
    )
    .await?;
    let state: EditableTriggerState = decode_row(rows.into_iter().next().ok_or_else(|| {
        ControlError::Invalid("trigger does not exist in the current snapshot".into())
    })?)?;
    let snapshot_id = state.snapshot_id;
    let (transaction, (before, after)) = apply_operator_change(transaction, state, request).await?;
    let transaction = append_audit(
        transaction,
        context,
        (request.event_id, "trigger.changed"),
        (
            json!({"trigger_id": request.trigger_id, "snapshot_id": snapshot_id, "request": body}),
            before,
            after,
        ),
    )
    .await?;
    Ok((transaction, true))
}
