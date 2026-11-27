//! Platform observations in storage order, filtered to the kinds a sensor watches; one that can't be read is passed over and kept for the tick to record.

use std::collections::BTreeSet;

use crate::platform::main::installation::read_stored_event::read_stored_event;
use crate::platform::models::{PlatformEventKind, StoredPlatformEvent};
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
    let (transaction, rows): (_, Vec<StoredPlatformEvent>) = query_records(
        transaction,
        &format!(
            "SELECT storage_id,event_id,occurred_at,event_type,payload FROM recall_events WHERE producer='platform' \
             AND invocation_id LIKE 'platform/%' AND {clause} LIMIT {EVENT_PAGE_SIZE}"
        ),
        vec![position],
    )
    .await?;
    let mut scanned = Scanned {
        full: rows.len() >= EVENT_PAGE_SIZE,
        ..Scanned::default()
    };
    for row in rows {
        scanned.last = Some(row.storage_id);
        let event = match read_stored_event(row) {
            Ok(event) => event,
            Err(row) => {
                let watched = kinds
                    .iter()
                    .any(|kind| row.event.event_type == format!("platform_{}", kind.as_str()));
                if watched {
                    scanned.unreadable.push(*row);
                }
                continue;
            }
        };
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
