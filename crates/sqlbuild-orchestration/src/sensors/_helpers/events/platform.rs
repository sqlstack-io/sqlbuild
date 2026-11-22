//! Platform observations in storage order, filtered to the kinds a sensor watches.

use std::collections::BTreeSet;

use crate::platform::models::{PlatformEventEnvelope, PlatformEventKind};
use crate::sensors::_helpers::events::conditions::page_clause;
use crate::sensors::constants::EVENT_PAGE_SIZE;
use crate::sensors::models::{Direction, Scanned, SensorEvent};
use crate::store::errors::ControlError;
use crate::store::main::query_records::query_records;
use crate::store::models::ControlTransaction;

pub(crate) async fn scan_platform_events(
    transaction: ControlTransaction,
    (direction, limit): (Direction, usize),
    kinds: &BTreeSet<PlatformEventKind>,
) -> Result<(ControlTransaction, Scanned), ControlError> {
    let (clause, position) = page_clause(direction, "storage_id", 1);
    let (transaction, events): (_, Vec<PlatformEventEnvelope>) = query_records(
        transaction,
        &format!(
            "SELECT storage_id,event_id,occurred_at,payload FROM recall_events WHERE producer='platform' \
             AND invocation_id LIKE 'platform/%' AND {clause} LIMIT {EVENT_PAGE_SIZE}"
        ),
        vec![position],
    )
    .await?;
    let mut scanned = Scanned {
        full: events.len() >= EVENT_PAGE_SIZE,
        ..Scanned::default()
    };
    for event in events {
        scanned.last = Some(event.storage_id);
        let kind = event.payload.platform.kind();
        if !kinds.contains(&kind) {
            continue;
        }
        let details = serde_json::to_value(&event.payload.platform)
            .map_err(|error| ControlError::Invalid(error.to_string()))?;
        scanned.events.push(SensorEvent::Platform {
            id: format!("platform:{}", event.event_id),
            event: kind,
            occurred_at: event.occurred_at,
            details,
        });
        if scanned.events.len() >= limit {
            break;
        }
    }
    Ok((transaction, scanned))
}
