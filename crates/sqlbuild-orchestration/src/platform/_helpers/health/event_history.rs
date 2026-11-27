//! Stored platform events newest first, each with the location target it concerns and who may see it; one that can't be read is listed on its own.

use std::collections::BTreeMap;

use serde_json::Value;
use uuid::Uuid;

use crate::authorization::main::decisions::person_holds_target::person_holds_target;
use crate::credentials::models::{Action, DefaultRole};
use crate::platform::models::{
    EventScope, PayloadScope, PersonPlatformEvent, PersonPlatformEventPage, PlatformEvent,
    PlatformEventEnvelope, PlatformEventPayload, StoredPlatformEvent, UnreadablePlatformEvent,
    UnreadableRow,
};
use crate::store::errors::ControlError;
use crate::store::main::{decode_row::decode_row, query::query, query_records::query_records};
use crate::store::models::ControlStore;
use crate::store::models::{ControlTransaction, Parameter};

/// Up to `limit` events before `before` without heartbeats, run failures or orphans, targets resolved, and where a further scan continues.
pub(crate) async fn event_history(
    transaction: ControlTransaction,
    (before, limit): (Option<i64>, u32),
) -> Result<(ControlTransaction, PersonPlatformEventPage), ControlError> {
    let limit: u32 = limit.clamp(1, 500);
    let mut parameters = vec![Parameter::Integer(i64::from(limit))];
    let older = before.map_or("", |storage_id| {
        parameters.push(Parameter::Integer(storage_id));
        "AND storage_id<$2 "
    });
    let (transaction, rows): (ControlTransaction, Vec<StoredPlatformEvent>) = query_records(transaction, &format!(
        "SELECT storage_id,event_id,occurred_at,event_type,payload FROM recall_events WHERE producer='platform' \
         AND invocation_id LIKE 'platform/%' AND event_type NOT IN ('platform_heartbeat','platform_run_failed') \
         {older}ORDER BY storage_id DESC LIMIT $1"),
        parameters).await?;
    let next: Option<i64> = (rows.len() == limit as usize)
        .then(|| rows.last().map(|row| row.storage_id))
        .flatten();
    let mut events: Vec<PlatformEventEnvelope> = Vec::new();
    let mut unreadable: Vec<UnreadableRow> = Vec::new();
    for row in rows {
        match read_event(row) {
            Ok(envelope) => events.push(envelope),
            Err(item) => unreadable.push(*item),
        }
    }
    let mut triggers: Vec<Uuid> = Vec::new();
    let mut runs: Vec<Uuid> = Vec::new();
    for envelope in &events {
        match envelope.payload.platform {
            PlatformEvent::TickOverdue { trigger_id, .. }
            | PlatformEvent::CalendarHorizonLow { trigger_id, .. }
            | PlatformEvent::TriggerSkipped { trigger_id, .. }
            | PlatformEvent::SensorRechecked { trigger_id, .. }
            | PlatformEvent::CalendarTimesDropped { trigger_id, .. } => triggers.push(trigger_id),
            PlatformEvent::LaunchFailed { run_id, .. } => runs.push(run_id),
            _ => {}
        }
    }
    triggers.extend(unreadable.iter().filter_map(|row| row.scope));
    runs.extend(unreadable.iter().filter_map(|row| row.scope));
    let (transaction, mut scopes) =
        scopes_of(transaction, ("sqb_triggers", "trigger_id"), &triggers).await?;
    let (transaction, run_scopes) = scopes_of(transaction, ("sqb_runs", "run_id"), &runs).await?;
    scopes.extend(run_scopes);
    let page = PersonPlatformEventPage {
        events: events
            .into_iter()
            .filter_map(|envelope| person_event(envelope, &scopes))
            .collect(),
        before: next,
        unreadable: unreadable
            .into_iter()
            .map(|item| scoped(item, &scopes))
            .collect(),
    };
    Ok((transaction, page))
}

/// The row's event, or what is known about it and why it can't be read.
pub(crate) fn read_event(
    row: StoredPlatformEvent,
) -> Result<PlatformEventEnvelope, Box<UnreadableRow>> {
    let payload: Value = match row.payload {
        Value::String(text) => serde_json::from_str(&text).unwrap_or(Value::String(text)),
        payload => payload,
    };
    match serde_json::from_value::<PlatformEventPayload>(payload.clone()) {
        Ok(payload) => Ok(PlatformEventEnvelope {
            storage_id: row.storage_id,
            event_id: row.event_id,
            occurred_at: row.occurred_at,
            payload,
        }),
        Err(error) => {
            let named: PayloadScope =
                serde_json::from_value(payload["platform"].clone()).unwrap_or_default();
            Err(Box::new(UnreadableRow {
                event: UnreadablePlatformEvent {
                    storage_id: row.storage_id,
                    event_id: row.event_id,
                    occurred_at: row.occurred_at,
                    event_type: row.event_type,
                    error: error.to_string(),
                    location_id: named.location_id,
                    target: named.target,
                },
                scope: named.trigger_id.or(named.run_id),
            }))
        }
    }
}

/// An unreadable event's location target: the one it names, else its trigger's or run's.
pub(crate) fn scoped(
    UnreadableRow {
        event: mut item,
        scope,
    }: UnreadableRow,
    scopes: &BTreeMap<Uuid, (Uuid, String)>,
) -> UnreadablePlatformEvent {
    if item.location_id.is_none() || item.target.is_none() {
        let resolved = scope.and_then(|id| scopes.get(&id).cloned());
        (item.location_id, item.target) = resolved.map_or((None, None), |(location_id, target)| {
            (Some(location_id), Some(target))
        });
    }
    item
}

fn person_event(
    envelope: PlatformEventEnvelope,
    scopes: &BTreeMap<Uuid, (Uuid, String)>,
) -> Option<PersonPlatformEvent> {
    let scope: Option<(Uuid, String)> = match &envelope.payload.platform {
        PlatformEvent::LocationBroken {
            location_id,
            target,
            ..
        }
        | PlatformEvent::LocationFailing {
            location_id,
            target,
            ..
        } => Some((*location_id, target.clone())),
        PlatformEvent::UnreadableEventSkipped {
            location_id,
            target,
            ..
        } => location_id.zip(target.clone()),
        PlatformEvent::TickOverdue { trigger_id, .. }
        | PlatformEvent::CalendarHorizonLow { trigger_id, .. }
        | PlatformEvent::TriggerSkipped { trigger_id, .. }
        | PlatformEvent::SensorRechecked { trigger_id, .. }
        | PlatformEvent::CalendarTimesDropped { trigger_id, .. } => {
            Some(scopes.get(trigger_id).cloned()?)
        }
        PlatformEvent::LaunchFailed { run_id, .. } => Some(scopes.get(run_id).cloned()?),
        PlatformEvent::RunFailed { .. } => return None,
        PlatformEvent::Heartbeat { .. } | PlatformEvent::SchedulerNoLeader => None,
    };
    let (location_id, target) = scope.map_or((None, None), |(location_id, target)| {
        (Some(location_id), Some(target))
    });
    Some(PersonPlatformEvent {
        storage_id: envelope.storage_id,
        event_id: envelope.event_id,
        occurred_at: envelope.occurred_at,
        location_id,
        target,
        event: envelope.payload.platform,
    })
}

/// The location target of each trigger or run in `ids`, read from `table`'s `column`.
pub(crate) async fn scopes_of(
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

/// Which location targets one person may view, asked once per target.
pub(crate) struct Visibility<'a> {
    pub(crate) store: &'a ControlStore,
    pub(crate) person: (Uuid, DefaultRole),
    pub(crate) listed: BTreeMap<(Uuid, String), bool>,
}

impl Visibility<'_> {
    /// Whether the person may view `scope`; server-wide events have none.
    pub(crate) async fn shown(
        &mut self,
        scope: (Option<Uuid>, Option<String>),
    ) -> Result<bool, ControlError> {
        let (Some(location_id), Some(target)) = scope else {
            return Ok(true);
        };
        if let Some(shown) = self.listed.get(&(location_id, target.clone())) {
            return Ok(*shown);
        }
        let (principal_id, default_role) = self.person;
        let shown = person_holds_target(
            self.store,
            principal_id,
            (Action::View, location_id, &target),
            default_role,
        )
        .await?;
        self.listed.insert((location_id, target), shown);
        Ok(shown)
    }
}
