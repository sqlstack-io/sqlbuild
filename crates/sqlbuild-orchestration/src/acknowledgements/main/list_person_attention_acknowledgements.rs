//! A condition's acknowledgement is its latest acknowledgement event, projected from the audit log.

use uuid::Uuid;

use crate::acknowledgements::_helpers::latest_events::{
    AcknowledgementEvents, latest_acknowledgements,
};
use crate::acknowledgements::constants::{
    ACKNOWLEDGED, WITHDRAWN, attention_listing_window, attention_snooze,
};
use crate::acknowledgements::models::AttentionAcknowledgement;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

const ATTENTION_EVENTS: AcknowledgementEvents = AcknowledgementEvents {
    acknowledged: ACKNOWLEDGED,
    withdrawn: WITHDRAWN,
    subject: "key",
};

/// Conditions whose latest event acknowledges them, from the last two days; a lapsed one is listed with
/// its `until`, so the Overview can say why the condition is back.
pub async fn list_person_attention_acknowledgements(
    store: &ControlStore,
    person: (&AuditContext, Uuid),
    (location_id, target): (Uuid, &str),
    default_role: DefaultRole,
) -> Result<Vec<AttentionAcknowledgement>, ControlError> {
    let latest = latest_acknowledgements(
        store,
        person,
        (location_id, target),
        default_role,
        (&ATTENTION_EVENTS, attention_listing_window()),
    )
    .await?;
    Ok(latest
        .into_iter()
        .map(|ack| AttentionAcknowledgement {
            shape: ack.scope["shape"].clone(),
            key: ack.subject,
            acknowledged_by: ack.acknowledged_by,
            acknowledged_at: ack.acknowledged_at,
            until: ack.acknowledged_at + attention_snooze(),
        })
        .collect())
}
