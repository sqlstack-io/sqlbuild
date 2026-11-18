//! A run's acknowledgement is its latest acknowledgement event, projected from the audit log.

use chrono::Duration;
use uuid::Uuid;

use crate::acknowledgements::_helpers::latest_events::{
    AcknowledgementEvents, latest_acknowledgements,
};
use crate::acknowledgements::models::RunAcknowledgement;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

const RUN_EVENTS: AcknowledgementEvents = AcknowledgementEvents {
    acknowledged: "run.acknowledged",
    withdrawn: "run.acknowledgement_withdrawn",
    subject: "run_id",
};

/// Runs marked handled in the last week; the overview lists only a day of problems.
pub async fn list_person_acknowledgements(
    store: &ControlStore,
    person: (&AuditContext, Uuid),
    (location_id, target): (Uuid, &str),
    default_role: DefaultRole,
) -> Result<Vec<RunAcknowledgement>, ControlError> {
    let latest = latest_acknowledgements(
        store,
        person,
        (location_id, target),
        default_role,
        (&RUN_EVENTS, Duration::days(7)),
    )
    .await?;
    latest
        .into_iter()
        .map(|ack| {
            Ok(RunAcknowledgement {
                run_id: ack.subject.parse().map_err(|error| {
                    ControlError::Schema(format!("acknowledgement run: {error}"))
                })?,
                acknowledged_by: ack.acknowledged_by,
                acknowledged_at: ack.acknowledged_at,
            })
        })
        .collect()
}
