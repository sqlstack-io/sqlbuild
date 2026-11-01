//! Apply the operator's projection and return the before/after audit images in the same transaction.

use serde_json::{Value, json};

use crate::declarations::_helpers::trigger_state::trigger_timing;
use crate::declarations::models::{
    EditableTriggerState, TriggerChange, TriggerChangeRequest, TriggerDeclaration, TriggerStatus,
};
use crate::sensors::main::read_sensor_cursor::read_sensor_cursor;
use crate::sensors::main::reset_sensor_cursor::reset_sensor_cursor;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn apply_operator_change(
    transaction: ControlTransaction,
    mut state: EditableTriggerState,
    request: &TriggerChangeRequest,
) -> Result<(ControlTransaction, (Value, Value)), ControlError> {
    let (mut transaction, cursor) = match &state.definition_json {
        TriggerDeclaration::Sensor(_) => {
            if matches!(request.change, TriggerChange::SetCursor { .. }) {
                return Err(ControlError::Invalid(
                    "sensor cursors use storage order; use reset-cursor to replay retained events"
                        .into(),
                ));
            }
            let (transaction, cursor) = read_sensor_cursor(transaction, request.trigger_id).await?;
            (transaction, Some(cursor))
        }
        TriggerDeclaration::Schedule(_) => (transaction, None),
    };
    let mut before =
        json!({"status_override": state.status_override, "last_tick_for": state.last_tick_for});
    if let Some(cursor) = &cursor {
        before["cursor"] = json!(cursor);
    }
    match request.change {
        TriggerChange::Enable => state.status_override = Some(TriggerStatus::Running),
        TriggerChange::Pause => state.status_override = Some(TriggerStatus::Paused),
        TriggerChange::ResetDefault => state.status_override = None,
        TriggerChange::SetCursor { time } => state.last_tick_for = Some(time),
        TriggerChange::ResetCursor => state.last_tick_for = None,
    }
    let (status, next) = match &state.definition_json {
        TriggerDeclaration::Schedule(definition) => trigger_timing(
            definition,
            state.started_at,
            state.last_tick_for,
            state.status_override,
        )?,
        TriggerDeclaration::Sensor(definition) => {
            if matches!(request.change, TriggerChange::ResetCursor) {
                transaction =
                    reset_sensor_cursor(transaction, (request.trigger_id, request.event_id))
                        .await?;
            }
            let status = state
                .status_override
                .unwrap_or(definition.default_status.projection());
            (
                status,
                (status == TriggerStatus::Running).then_some(transaction.now),
            )
        }
    };
    let mut after = json!({"status_override": state.status_override, "last_tick_for": state.last_tick_for, "reason": request.reason});
    if let Some(cursor) = cursor {
        after["cursor"] = json!(if matches!(request.change, TriggerChange::ResetCursor) {
            Default::default()
        } else {
            cursor
        });
    }
    let now = transaction.now;
    let (transaction, _) = query(transaction,
        "UPDATE sqb_trigger_state SET status=$1, status_override=$2, last_tick_for=$3, next_due_at=$4, updated_at=$5 WHERE trigger_id=$6",
        vec![Parameter::Text(status.as_str().into()), Parameter::OptionalText(state.status_override.map(|value| value.as_str().into())), Parameter::OptionalTime(state.last_tick_for),
            Parameter::OptionalTime(next), Parameter::Time(now), Parameter::Id(request.trigger_id)]).await?;
    Ok((transaction, (before, after)))
}
