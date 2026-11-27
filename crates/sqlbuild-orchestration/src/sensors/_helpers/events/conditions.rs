//! Each condition reads its matching events a bounded page at a time, forward for ticks or back for a person's look.

use crate::platform::main::health::record_skipped_event::record_skipped_event;
use crate::sensors::_helpers::events::nodes::{scan_check_events, scan_node_events};
use crate::sensors::_helpers::events::platform::scan_platform_events;
use crate::sensors::_helpers::events::runs::scan_run_events;
use crate::sensors::_helpers::events::sources::{next_source_event, recent_source_events};
use crate::sensors::constants::RECENT_EVENT_PAGES;
use crate::sensors::models::{
    Direction, EventScope, NextEvent, Scanned, SensorCondition, SensorCursor, SensorEvent,
};
use crate::store::errors::ControlError;
use crate::store::models::{ControlTransaction, Parameter};

/// The oldest event after `cursor` the sensor has not handled; a watched one that can't be read is passed over and recorded as a problem, so the tick writes.
pub(crate) async fn next_event(
    transaction: ControlTransaction,
    scope: EventScope<'_>,
    (condition, cursor): (&SensorCondition, &SensorCursor),
) -> Result<(ControlTransaction, NextEvent), ControlError> {
    if matches!(
        condition,
        SensorCondition::SourceNewData { .. } | SensorCondition::SourceStale { .. }
    ) {
        return next_source_event(transaction, scope, (condition, cursor)).await;
    }
    let (mut transaction, mut scanned) = scan(
        transaction,
        scope,
        condition,
        (Direction::After(cursor.event_storage_id), 1),
    )
    .await?;
    for row in std::mem::take(&mut scanned.unreadable) {
        transaction = record_skipped_event(transaction, row).await?;
    }
    let after = SensorCursor {
        event_storage_id: scanned.last.unwrap_or(cursor.event_storage_id),
        ..cursor.clone()
    };
    Ok((
        transaction,
        match scanned.events.pop() {
            Some(event) => NextEvent::Found {
                event,
                cursor_after: after,
            },
            None => NextEvent::Nothing {
                cursor: after,
                more: scanned.full,
            },
        },
    ))
}

/// Up to `limit` of the newest events matching `condition`, newest first, whether or not the sensor handled them.
pub(crate) async fn recent_events(
    mut transaction: ControlTransaction,
    scope: EventScope<'_>,
    (condition, limit): (&SensorCondition, usize),
) -> Result<(ControlTransaction, Vec<SensorEvent>), ControlError> {
    if matches!(
        condition,
        SensorCondition::SourceNewData { .. } | SensorCondition::SourceStale { .. }
    ) {
        return recent_source_events(transaction, scope, (condition, limit)).await;
    }
    let mut events: Vec<SensorEvent> = Vec::new();
    let mut before: i64 = i64::MAX;
    for _ in 0..RECENT_EVENT_PAGES {
        let scanned: Scanned;
        (transaction, scanned) = scan(
            transaction,
            scope,
            condition,
            (Direction::Before(before), limit - events.len()),
        )
        .await?;
        events.extend(scanned.events);
        match scanned.last {
            Some(last) if scanned.full && events.len() < limit => before = last,
            _ => break,
        }
    }
    Ok((transaction, events))
}

/// One page of a storage-ordered condition's matching events, at most `limit`.
async fn scan(
    transaction: ControlTransaction,
    scope: EventScope<'_>,
    condition: &SensorCondition,
    page: (Direction, usize),
) -> Result<(ControlTransaction, Scanned), ControlError> {
    match condition {
        SensorCondition::PlatformEvent { kinds } => {
            scan_platform_events(transaction, page, kinds).await
        }
        SensorCondition::NodeEvent { selection, kinds } => {
            scan_node_events(transaction, scope, page, (selection, kinds)).await
        }
        SensorCondition::CheckEvent { selection, kinds } => {
            scan_check_events(transaction, scope, page, (selection, kinds)).await
        }
        SensorCondition::RunEvent {
            job_key,
            kinds,
            executed_successfully,
        } => {
            scan_run_events(
                transaction,
                scope,
                page,
                (job_key, kinds, executed_successfully.as_ref()),
            )
            .await
        }
        SensorCondition::SourceNewData { .. } | SensorCondition::SourceStale { .. } => Err(
            ControlError::Invalid("source conditions have no storage order".into()),
        ),
    }
}

/// The predicate and order a page reads `column` in, with the position as parameter `$index`.
pub(crate) fn page_clause(direction: Direction, column: &str, index: usize) -> (String, Parameter) {
    match direction {
        Direction::After(position) => (
            format!("{column}>${index} ORDER BY {column}"),
            Parameter::Integer(position),
        ),
        Direction::Before(position) => (
            format!("{column}<${index} ORDER BY {column} DESC"),
            Parameter::Integer(position),
        ),
    }
}

/// The UI page of `run_id`, when the server knows the address people reach it at.
pub(crate) fn run_url(origin: Option<&str>, run_id: uuid::Uuid) -> Option<String> {
    origin.map(|origin| format!("{}/runs/{run_id}", origin.trim_end_matches('/')))
}
