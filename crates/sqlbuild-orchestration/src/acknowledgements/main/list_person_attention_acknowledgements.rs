//! A condition's acknowledgement is its latest acknowledgement event, projected from the audit log.

use chrono::Duration;
use uuid::Uuid;

use crate::acknowledgements::_helpers::latest_events::latest_acknowledgements;
use crate::acknowledgements::constants::{
    ATTENTION_EVENTS, ATTENTION_LISTING_HOURS, ATTENTION_SNOOZE_HOURS,
};
use crate::acknowledgements::models::AcknowledgementListing;
use crate::acknowledgements::models::AttentionAcknowledgement;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

/// Conditions acknowledged in the last two days, lapsed ones included with their `until`.
pub async fn list_person_attention_acknowledgements(
    store: &ControlStore,
    person: (&AuditContext, Uuid),
    (location_id, target): (Uuid, &str),
    default_role: DefaultRole,
) -> Result<Vec<AttentionAcknowledgement>, ControlError> {
    let listing = AcknowledgementListing {
        location_id,
        target,
        default_role,
        events: &ATTENTION_EVENTS,
        window: Duration::hours(ATTENTION_LISTING_HOURS),
    };
    Ok(latest_acknowledgements(store, person, listing)
        .await?
        .into_iter()
        .map(|ack| AttentionAcknowledgement {
            shape: ack.scope["shape"].clone(),
            key: ack.subject,
            acknowledged_by: ack.acknowledged_by,
            acknowledged_at: ack.acknowledged_at,
            until: ack.acknowledged_at + Duration::hours(ATTENTION_SNOOZE_HOURS),
        })
        .collect())
}
