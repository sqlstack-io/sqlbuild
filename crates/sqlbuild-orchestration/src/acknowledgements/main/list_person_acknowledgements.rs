//! A run's acknowledgement is its latest acknowledgement event, projected from the audit log.

use chrono::Duration;
use uuid::Uuid;

use crate::acknowledgements::_helpers::latest_events::latest_acknowledgements;
use crate::acknowledgements::constants::{RUN_EVENTS, RUN_LISTING_DAYS};
use crate::acknowledgements::models::AcknowledgementListing;
use crate::acknowledgements::models::RunAcknowledgement;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

/// Runs marked handled in the last week; the overview lists only a day of problems.
pub async fn list_person_acknowledgements(
    store: &ControlStore,
    person: (&AuditContext, Uuid),
    (location_id, target): (Uuid, &str),
    default_role: DefaultRole,
) -> Result<Vec<RunAcknowledgement>, ControlError> {
    let listing = AcknowledgementListing {
        location_id,
        target,
        default_role,
        events: &RUN_EVENTS,
        window: Duration::days(RUN_LISTING_DAYS),
    };
    latest_acknowledgements(store, person, listing)
        .await?
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
