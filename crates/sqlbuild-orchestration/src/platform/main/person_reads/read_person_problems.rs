//! People see the scheduler's state and stored events nothing could read, and broken locations, late ticks, low calendars, skipped triggers and re-checked sensors only for targets they list.

use std::collections::BTreeSet;

use uuid::Uuid;

use crate::authorization::main::decisions::person_holds_target::person_holds_target;
use crate::credentials::models::{Action, DefaultRole};
use crate::platform::_helpers::health::problems::{late_ticks, scan_problems};
use crate::platform::models::PlatformProblems;
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

/// Access is checked per target before that target's late ticks are read, so one target cannot crowd out another.
pub async fn read_person_problems(
    store: &ControlStore,
    principal_id: Uuid,
    default_role: DefaultRole,
) -> Result<PlatformProblems, ControlError> {
    let scan = scan_problems(store).await?;
    let mut listed: BTreeSet<(Uuid, String)> = BTreeSet::new();
    for (location_id, target) in scan
        .broken
        .iter()
        .map(|location| (location.location_id, location.target.clone()))
        .chain(
            scan.failing
                .iter()
                .map(|location| (location.location_id, location.target.clone())),
        )
        .chain(
            scan.stalled_syncs
                .iter()
                .map(|sync| (sync.location_id, sync.target.clone())),
        )
        .chain(scan.late_targets.iter().cloned())
        .chain(
            scan.low_calendars
                .iter()
                .map(|calendar| (calendar.location_id, calendar.target.clone())),
        )
        .chain(
            scan.skipped_triggers
                .iter()
                .map(|trigger| (trigger.location_id, trigger.target.clone())),
        )
        .chain(
            scan.rechecked_sensors
                .iter()
                .map(|sensor| (sensor.location_id, sensor.target.clone())),
        )
        .chain(
            scan.dropped_calendar_times
                .iter()
                .map(|calendar| (calendar.location_id, calendar.target.clone())),
        )
        .chain(
            scan.skipped_events
                .iter()
                .filter_map(|event| event.location_id.zip(event.target.clone())),
        )
        .collect::<BTreeSet<_>>()
    {
        if person_holds_target(
            store,
            principal_id,
            (Action::View, location_id, &target),
            default_role,
        )
        .await?
        {
            listed.insert((location_id, target));
        }
    }
    Ok(PlatformProblems {
        scheduler_leader: scan
            .lease
            .as_ref()
            .is_some_and(|lease| lease.expires_at > scan.now),
        scheduler_lease_expires_at: scan.lease.map(|lease| lease.expires_at),
        overdue_ticks: late_ticks(store, scan.late_targets.intersection(&listed), scan.now).await?,
        broken_locations: scan
            .broken
            .into_iter()
            .filter(|location| listed.contains(&(location.location_id, location.target.clone())))
            .collect(),
        failing_locations: scan
            .failing
            .into_iter()
            .filter(|location| listed.contains(&(location.location_id, location.target.clone())))
            .collect(),
        failing_syncs: scan
            .failing_syncs
            .into_iter()
            .filter(|sync| listed.contains(&(sync.location_id, sync.target.clone())))
            .collect(),
        stalled_syncs: scan
            .stalled_syncs
            .into_iter()
            .filter(|sync| listed.contains(&(sync.location_id, sync.target.clone())))
            .collect(),
        failing_sinks: scan
            .failing_sinks
            .into_iter()
            .filter(|sink| listed.contains(&(sink.location_id, sink.target.clone())))
            .collect(),
        failing_forwarding: Vec::new(),
        low_calendars: scan
            .low_calendars
            .into_iter()
            .filter(|calendar| listed.contains(&(calendar.location_id, calendar.target.clone())))
            .collect(),
        skipped_triggers: scan
            .skipped_triggers
            .into_iter()
            .filter(|trigger| listed.contains(&(trigger.location_id, trigger.target.clone())))
            .collect(),
        rechecked_sensors: scan
            .rechecked_sensors
            .into_iter()
            .filter(|sensor| listed.contains(&(sensor.location_id, sensor.target.clone())))
            .collect(),
        dropped_calendar_times: scan
            .dropped_calendar_times
            .into_iter()
            .filter(|calendar| listed.contains(&(calendar.location_id, calendar.target.clone())))
            .collect(),
        skipped_events: scan
            .skipped_events
            .into_iter()
            .filter(|event| {
                event
                    .location_id
                    .zip(event.target.clone())
                    .is_none_or(|scope| listed.contains(&scope))
            })
            .collect(),
    })
}
