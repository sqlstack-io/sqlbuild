//! Webhook delivery and platform sensors record a stored event they skip because it can't be read, once per event.

use uuid::Uuid;

use crate::platform::_helpers::health::event_history::{scoped, scopes_of};
use crate::platform::main::health::record_trigger_problem::record_trigger_problem;
use crate::platform::models::{PlatformEvent, UnreadableRow};
use crate::store::errors::ControlError;
use crate::store::models::ControlTransaction;

/// Record the skipped event with its location target, its own or its trigger's or run's; skipping it again, by any reader or release, adds nothing.
pub(crate) async fn record_skipped_event(
    transaction: ControlTransaction,
    row: UnreadableRow,
) -> Result<ControlTransaction, ControlError> {
    let ids: Vec<Uuid> = row.scope.into_iter().collect();
    let (transaction, mut scopes) =
        scopes_of(transaction, ("sqb_triggers", "trigger_id"), &ids).await?;
    let (transaction, runs) = scopes_of(transaction, ("sqb_runs", "run_id"), &ids).await?;
    scopes.extend(runs);
    let item = scoped(row, &scopes);
    let episode = PlatformEvent::UnreadableEventSkipped {
        storage_id: item.storage_id,
        event_id: item.event_id,
        event_type: item.event_type.clone(),
        location_id: None,
        target: None,
        reason: String::new(),
    };
    let event = PlatformEvent::UnreadableEventSkipped {
        storage_id: item.storage_id,
        event_id: item.event_id,
        event_type: item.event_type,
        location_id: item.location_id,
        target: item.target,
        reason: item.error,
    };
    record_trigger_problem(transaction, (event, Some(episode), None)).await
}
