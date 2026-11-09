//! Stored platform events newest first, each with the location target it concerns.

use std::collections::BTreeMap;

use uuid::Uuid;

use crate::platform::models::{
    EventScope, PersonPlatformEvent, PlatformEvent, PlatformEventEnvelope,
};
use crate::store::errors::ControlError;
use crate::store::main::{decode_row::decode_row, query::query, query_records::query_records};
use crate::store::models::{ControlTransaction, Parameter};

/// Up to `limit` events before `before`, heartbeats left out, with their location targets resolved.
pub(crate) async fn event_history(
    transaction: ControlTransaction,
    (before, limit): (Option<i64>, u32),
) -> Result<(ControlTransaction, Vec<PersonPlatformEvent>), ControlError> {
    let mut parameters = vec![Parameter::Integer(i64::from(limit.clamp(1, 500)))];
    let older = before.map_or("", |storage_id| {
        parameters.push(Parameter::Integer(storage_id));
        "AND storage_id<$2 "
    });
    let (transaction, events): (ControlTransaction, Vec<PlatformEventEnvelope>) = query_records(transaction, &format!(
        "SELECT storage_id,event_id,occurred_at,payload FROM recall_events WHERE producer='platform' \
         AND invocation_id LIKE 'platform/%' AND event_type<>'platform_heartbeat' {older}ORDER BY storage_id DESC LIMIT $1"),
        parameters).await?;
    let mut triggers: Vec<Uuid> = Vec::new();
    let mut runs: Vec<Uuid> = Vec::new();
    for envelope in &events {
        match envelope.payload.platform {
            PlatformEvent::TickOverdue { trigger_id, .. }
            | PlatformEvent::CalendarHorizonLow { trigger_id, .. } => triggers.push(trigger_id),
            PlatformEvent::LaunchFailed { run_id, .. } => runs.push(run_id),
            _ => {}
        }
    }
    let (transaction, mut scopes) =
        scopes_of(transaction, ("sqb_triggers", "trigger_id"), &triggers).await?;
    let (transaction, run_scopes) = scopes_of(transaction, ("sqb_runs", "run_id"), &runs).await?;
    scopes.extend(run_scopes);
    let history: Vec<PersonPlatformEvent> = events
        .into_iter()
        .map(|envelope| person_event(envelope, &scopes))
        .collect();
    Ok((transaction, history))
}

fn person_event(
    envelope: PlatformEventEnvelope,
    scopes: &BTreeMap<Uuid, (Uuid, String)>,
) -> PersonPlatformEvent {
    let scope: Option<(Uuid, String)> = match &envelope.payload.platform {
        PlatformEvent::LocationBroken {
            location_id,
            target,
            ..
        } => Some((*location_id, target.clone())),
        PlatformEvent::TickOverdue { trigger_id, .. }
        | PlatformEvent::CalendarHorizonLow { trigger_id, .. } => scopes.get(trigger_id).cloned(),
        PlatformEvent::LaunchFailed { run_id, .. } => scopes.get(run_id).cloned(),
        PlatformEvent::Heartbeat { .. } | PlatformEvent::SchedulerNoLeader => None,
    };
    let (location_id, target) = scope.map_or((None, None), |(location_id, target)| {
        (Some(location_id), Some(target))
    });
    PersonPlatformEvent {
        storage_id: envelope.storage_id,
        event_id: envelope.event_id,
        occurred_at: envelope.occurred_at,
        location_id,
        target,
        event: envelope.payload.platform,
    }
}

async fn scopes_of(
    transaction: ControlTransaction,
    (table, column): (&str, &str),
    ids: &[Uuid],
) -> Result<(ControlTransaction, BTreeMap<Uuid, (Uuid, String)>), ControlError> {
    if ids.is_empty() {
        return Ok((transaction, BTreeMap::new()));
    }
    let (transaction, rows) = query(
        transaction,
        &format!(
            "SELECT {column} AS id, location_id, target FROM {table} WHERE {column} IN ({})",
            Parameter::placeholders(1..=ids.len())
        ),
        ids.iter().copied().map(Parameter::Id).collect(),
    )
    .await?;
    let scopes: BTreeMap<Uuid, (Uuid, String)> = rows
        .into_iter()
        .map(decode_row)
        .collect::<Result<Vec<EventScope>, _>>()?
        .into_iter()
        .map(|scope| (scope.id, (scope.location_id, scope.target)))
        .collect();
    Ok((transaction, scopes))
}
