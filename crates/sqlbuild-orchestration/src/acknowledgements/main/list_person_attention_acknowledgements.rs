//! A condition's acknowledgement is its latest acknowledgement event, projected from the audit log.

use uuid::Uuid;

use crate::acknowledgements::_helpers::latest_events::latest_acknowledgements;
use crate::acknowledgements::constants::ATTENTION_EVENTS;
use crate::acknowledgements::models::{
    AcknowledgementListing, AttentionAcknowledgement, AttentionAcknowledgements, AttentionSnooze,
};
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

/// Conditions acknowledged within two snoozes, lapsed ones included with their `until`.
pub async fn list_person_attention_acknowledgements(
    store: &ControlStore,
    person: (&AuditContext, Uuid),
    (location_id, target): (Uuid, &str),
    (default_role, snooze): (DefaultRole, AttentionSnooze),
) -> Result<AttentionAcknowledgements, ControlError> {
    let listing = AcknowledgementListing {
        location_id,
        target,
        default_role,
        events: &ATTENTION_EVENTS,
        window: snooze.listing_window(),
    };
    let acknowledgements: Vec<AttentionAcknowledgement> =
        latest_acknowledgements(store, person, listing)
            .await?
            .into_iter()
            .map(|ack| AttentionAcknowledgement {
                shape: ack.scope["shape"].clone(),
                key: ack.subject,
                acknowledged_by: ack.acknowledged_by,
                acknowledged_at: ack.acknowledged_at,
                until: snooze.until(ack.acknowledged_at),
            })
            .collect();
    Ok(AttentionAcknowledgements {
        snooze_seconds: snooze.seconds(),
        acknowledgements,
    })
}
