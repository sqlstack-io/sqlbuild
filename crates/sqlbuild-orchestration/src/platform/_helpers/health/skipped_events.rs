//! Stored events webhook delivery or a platform sensor skipped because they can't be read, while they are kept, still can't be read and nobody dismissed them.

use std::collections::{BTreeMap, BTreeSet};

use uuid::Uuid;

use crate::acknowledgements::constants::SKIP_DISMISSED;
use crate::platform::_helpers::health::event_history::read_event;
use crate::platform::constants::SKIPPED_TRIGGER_EVENTS;
use crate::platform::models::{
    ActiveLocation, DismissedSkip, PlatformEvent, PlatformEventEnvelope, SkippedEvent,
    StoredPlatformEvent,
};
use crate::store::errors::ControlError;
use crate::store::main::{decode_row::decode_row, query::query, query_records::query_records};
use crate::store::models::{ControlTransaction, Parameter};

/// Each recorded skip, newest first, whose event is still stored, still can't be read and isn't dismissed; one at an archived location is dropped.
pub(crate) async fn skipped_events(
    transaction: ControlTransaction,
) -> Result<(ControlTransaction, Vec<SkippedEvent>), ControlError> {
    let (transaction, recorded) = recorded_skips(transaction).await?;
    let (transaction, dismissed) = dismissed_skips(transaction).await?;
    let skipped: Vec<SkippedEvent> = recorded
        .into_iter()
        .filter(|event| !dismissed.contains(&event.storage_id))
        .collect();
    if skipped.is_empty() {
        return Ok((transaction, skipped));
    }
    open_skips(transaction, skipped).await
}

/// Every recorded skip the scan reads, newest first, whether or not it is still open.
pub(crate) async fn recorded_skips(
    transaction: ControlTransaction,
) -> Result<(ControlTransaction, Vec<SkippedEvent>), ControlError> {
    let (transaction, skips): (ControlTransaction, Vec<PlatformEventEnvelope>) = query_records(
        transaction,
        "SELECT storage_id,event_id,occurred_at,payload FROM recall_events WHERE producer='platform' \
         AND event_type=$2 ORDER BY storage_id DESC LIMIT $1",
        vec![
            Parameter::Integer(SKIPPED_TRIGGER_EVENTS),
            Parameter::Text("platform_unreadable_event_skipped".to_owned()),
        ],
    )
    .await?;
    let mut skipped: Vec<SkippedEvent> = Vec::new();
    for skip in skips {
        if let PlatformEvent::UnreadableEventSkipped {
            storage_id,
            event_id,
            event_type,
            location_id,
            target,
            reason,
        } = skip.payload.platform
        {
            skipped.push(SkippedEvent {
                storage_id,
                event_id,
                event_type,
                location_id,
                location_name: None,
                target,
                skipped_at: skip.occurred_at,
                reason,
            });
        }
    }
    Ok((transaction, skipped))
}

/// The stored events whose skip someone dismissed.
pub(crate) async fn dismissed_skips(
    transaction: ControlTransaction,
) -> Result<(ControlTransaction, BTreeSet<i64>), ControlError> {
    let (transaction, rows) = query(
        transaction,
        "SELECT scope_json FROM sqb_security_audit WHERE action=$1",
        vec![Parameter::Text(SKIP_DISMISSED.to_owned())],
    )
    .await?;
    let mut dismissed: BTreeSet<i64> = BTreeSet::new();
    for row in rows {
        let scope: DismissedSkip = serde_json::from_value(row["scope_json"].clone())
            .map_err(|error| ControlError::Schema(format!("skipped event dismissal: {error}")))?;
        dismissed.insert(scope.storage_id);
    }
    Ok((transaction, dismissed))
}

/// The skips whose event is still stored and still can't be read, with their location's name.
async fn open_skips(
    transaction: ControlTransaction,
    skipped: Vec<SkippedEvent>,
) -> Result<(ControlTransaction, Vec<SkippedEvent>), ControlError> {
    let (transaction, rows): (ControlTransaction, Vec<StoredPlatformEvent>) = query_records(
        transaction,
        &format!(
            "SELECT storage_id,event_id,occurred_at,event_type,payload FROM recall_events \
             WHERE storage_id IN ({})",
            Parameter::placeholders(1..=skipped.len())
        ),
        skipped
            .iter()
            .map(|event| Parameter::Integer(event.storage_id))
            .collect(),
    )
    .await?;
    let unreadable: BTreeMap<i64, Uuid> = rows
        .into_iter()
        .filter_map(|row| read_event(row).err())
        .map(|row| (row.event.storage_id, row.event.event_id))
        .collect();
    let (transaction, locations) = query(
        transaction,
        "SELECT location_id, name FROM sqb_locations WHERE archived_at IS NULL",
        vec![],
    )
    .await?;
    let mut names: BTreeMap<Uuid, String> = BTreeMap::new();
    for location in locations {
        let location: ActiveLocation = decode_row(location)?;
        names.insert(location.location_id, location.name);
    }
    let open: Vec<SkippedEvent> = skipped
        .into_iter()
        .filter(|event| unreadable.get(&event.storage_id) == Some(&event.event_id))
        .filter_map(|mut event| {
            if let Some(location_id) = event.location_id {
                event.location_name = Some(names.get(&location_id)?.clone());
            }
            Some(event)
        })
        .collect();
    Ok((transaction, open))
}
