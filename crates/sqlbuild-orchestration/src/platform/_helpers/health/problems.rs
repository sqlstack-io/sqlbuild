//! Live problem queries, with the watchdog's thresholds; callers decide who may see each target's.

use chrono::{DateTime, TimeDelta, Utc};
use uuid::Uuid;

use crate::declarations::models::Schedule;
use crate::platform::_helpers::health::failing_locations::failing_locations;
use crate::platform::_helpers::health::failing_sinks::{failing_sinks, sink_reports};
use crate::platform::_helpers::health::skipped_events::skipped_events;
use crate::platform::constants::LOCATION_SILENCE_SECONDS;
use std::collections::BTreeSet;

use crate::platform::constants::{OBSERVATION_WINDOW_SECONDS, SKIPPED_TRIGGER_EVENTS};
use crate::platform::models::{
    DroppedCalendarTimes, LateTarget, LocationFact, LowCalendar, OverdueTick, PlatformEvent,
    PlatformEventEnvelope, ProblemScan, PublishedTrigger, RecheckedSensor, RunningSchedule,
    ScopedProblems, SkippedTrigger, TriggerProblems,
};
use crate::store::errors::ControlError;
use crate::store::main::decode_row::decode_row;
use crate::store::main::query::query;
use crate::store::main::query_records::query_records;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, ControlTransaction, Parameter, TransactionMode};
use crate::store::types::Row;

pub(crate) async fn scan_problems(store: &ControlStore) -> Result<ProblemScan, ControlError> {
    let transaction = begin(store, TransactionMode::Read).await?;
    let now = transaction.now;
    let (transaction, leases) = query(
        transaction,
        "SELECT expires_at FROM sqb_scheduler_lease WHERE name='scheduler'",
        vec![],
    )
    .await?;
    let (transaction, locations) = query(
        transaction,
        "SELECT i.location_id, l.name AS location_name, i.target, MAX(i.last_seen_at) AS last_seen_at \
         FROM sqb_location_instances i JOIN sqb_locations l ON l.location_id=i.location_id \
         WHERE l.archived_at IS NULL GROUP BY i.location_id,l.name,i.target \
         HAVING MAX(i.last_seen_at)<$1 ORDER BY i.location_id,i.target",
        vec![Parameter::Time(now - TimeDelta::seconds(LOCATION_SILENCE_SECONDS))],
    )
    .await?;
    let (transaction, failing) = failing_locations(transaction).await?;
    let (transaction, scoped) = location_problems(transaction, None).await?;
    commit(transaction).await?;
    Ok(ProblemScan {
        low_calendars: scoped.low_calendars,
        skipped_triggers: scoped.triggers.skipped,
        rechecked_sensors: scoped.triggers.rechecked,
        dropped_calendar_times: scoped.triggers.dropped,
        skipped_events: scoped.skipped_events,
        now,
        lease: leases.into_iter().next().map(decode_row).transpose()?,
        broken: locations
            .into_iter()
            .map(|row| decode_row(tiered(row, LocationFact::NotCheckingIn)))
            .collect::<Result<_, _>>()?,
        failing: failing.locations,
        failing_syncs: failing.syncs,
        stalled_syncs: failing.stalled,
        failing_sinks: scoped.sinks,
        late_targets: scoped.late_targets,
    })
}

/// The open problems of `scope`'s target, or every target's, besides its state and sync, such as late schedules.
pub(crate) async fn location_problems(
    transaction: ControlTransaction,
    scope: Option<(Uuid, &str)>,
) -> Result<(ControlTransaction, ScopedProblems), ControlError> {
    let now = transaction.now;
    let within = |location_id: Uuid, target: &str| {
        scope.is_none_or(|(scoped, scoped_target)| scoped == location_id && scoped_target == target)
    };
    let (transaction, late_targets) = query(
        transaction,
        "SELECT DISTINCT t.location_id, t.target FROM sqb_triggers t \
         JOIN sqb_trigger_state s ON s.trigger_id=t.trigger_id \
         JOIN sqb_locations l ON l.location_id=t.location_id \
         WHERE t.kind='schedule' AND s.status='running' AND t.removed_in_snapshot_id IS NULL \
         AND l.archived_at IS NULL AND s.next_due_at<$1",
        vec![Parameter::Time(overdue_before(now))],
    )
    .await?;
    let (transaction, schedules) = query(
        transaction,
        "SELECT t.trigger_id, t.name, t.location_id, l.name AS location_name, t.target, t.definition_json \
         FROM sqb_triggers t JOIN sqb_trigger_state s ON s.trigger_id=t.trigger_id \
         JOIN sqb_locations l ON l.location_id=t.location_id \
         WHERE t.kind='schedule' AND s.status='running' AND t.removed_in_snapshot_id IS NULL \
         AND l.archived_at IS NULL ORDER BY l.name, t.target, t.name",
        vec![],
    )
    .await?;
    let (transaction, triggers) = trigger_problems(transaction).await?;
    let (transaction, sinks) = sink_reports(transaction).await?;
    let (transaction, skipped) = skipped_events(transaction).await?;
    let schedules: Vec<RunningSchedule> = schedules
        .into_iter()
        .map(decode_row)
        .collect::<Result<_, _>>()?;
    let low_calendars: Vec<LowCalendar> = schedules
        .into_iter()
        .filter(|schedule| within(schedule.location_id, &schedule.target))
        .filter_map(|schedule| {
            let (refresh_at, horizon_end) = calendar_low(&schedule.definition_json.schedule, now)?;
            Some(LowCalendar {
                trigger_id: schedule.trigger_id,
                name: schedule.name,
                location_id: schedule.location_id,
                location_name: schedule.location_name,
                target: schedule.target,
                refresh_at,
                horizon_end,
                tier: LocationFact::CalendarLow.problem_tier(),
            })
        })
        .collect();
    let problems = ScopedProblems {
        late_targets: late_targets
            .into_iter()
            .map(|row| {
                let target: LateTarget = decode_row(row)?;
                Ok((target.location_id, target.target))
            })
            .collect::<Result<BTreeSet<_>, ControlError>>()?
            .into_iter()
            .filter(|(location_id, target)| within(*location_id, target))
            .collect(),
        low_calendars,
        triggers: TriggerProblems {
            skipped: triggers
                .skipped
                .into_iter()
                .filter(|trigger| within(trigger.location_id, &trigger.target))
                .collect(),
            rechecked: triggers
                .rechecked
                .into_iter()
                .filter(|sensor| within(sensor.location_id, &sensor.target))
                .collect(),
            dropped: triggers
                .dropped
                .into_iter()
                .filter(|calendar| within(calendar.location_id, &calendar.target))
                .collect(),
        },
        sinks: failing_sinks(&sinks)
            .into_iter()
            .filter(|sink| within(sink.location_id, &sink.target))
            .collect(),
        skipped_events: skipped
            .into_iter()
            .filter(|event| {
                scope.is_none()
                    || event
                        .location_id
                        .zip(event.target.as_deref())
                        .is_some_and(|(location_id, target)| within(location_id, target))
            })
            .collect(),
    };
    Ok((transaction, problems))
}

/// `row` with the tier of an open problem of kind `fact`, which the server decides in one place.
fn tiered(mut row: Row, fact: LocationFact) -> Row {
    row.insert("tier".to_owned(), serde_json::json!(fact.problem_tier()));
    row
}

/// Each trigger's live problems, all for the snapshot it is still published in: skipped until the location publishes again or, for a sensor, a later check completes; a sensor's re-check from scratch until it or a later check completes; and dropped calendar times until the next successful tick.
async fn trigger_problems(
    transaction: ControlTransaction,
) -> Result<(ControlTransaction, TriggerProblems), ControlError> {
    let (transaction, skipped) = open_problems(
        transaction,
        "platform_trigger_skipped",
        "NOT EXISTS (SELECT 1 FROM sqb_sensor_evaluation_events e WHERE e.trigger_id=t.trigger_id \
         AND e.phase='completed' AND e.occurred_at>=$2)",
    )
    .await?;
    let (transaction, rechecked) = open_problems(
        transaction,
        "platform_sensor_rechecked",
        "NOT EXISTS (SELECT 1 FROM sqb_sensor_evaluation_events e WHERE e.trigger_id=t.trigger_id \
         AND e.phase='completed' AND e.evaluation_id>=$2)",
    )
    .await?;
    let (transaction, dropped) = open_problems(
        transaction,
        "platform_calendar_times_dropped",
        "NOT EXISTS (SELECT 1 FROM sqb_ticks k WHERE k.trigger_id=t.trigger_id \
         AND k.outcome IN ('requested','covered') AND k.evaluated_at>=p.published_at)",
    )
    .await?;
    let mut problems: TriggerProblems = TriggerProblems::default();
    for (envelope, trigger) in skipped.into_iter().chain(rechecked).chain(dropped) {
        let PublishedTrigger {
            trigger_id,
            name,
            location_id,
            location_name,
            target,
            ..
        } = trigger;
        let at = envelope.occurred_at;
        match envelope.payload.platform {
            PlatformEvent::TriggerSkipped { reason, .. } => problems.skipped.push(SkippedTrigger {
                trigger_id,
                name,
                location_id,
                location_name,
                target,
                skipped_at: at,
                reason,
                tier: LocationFact::TriggerSkipped.problem_tier(),
            }),
            PlatformEvent::SensorRechecked { .. } => problems.rechecked.push(RecheckedSensor {
                trigger_id,
                name,
                location_id,
                location_name,
                target,
                rechecked_at: at,
                tier: LocationFact::SensorRechecked.problem_tier(),
            }),
            PlatformEvent::CalendarTimesDropped { times, .. } => {
                problems.dropped.push(DroppedCalendarTimes {
                    trigger_id,
                    name,
                    location_id,
                    location_name,
                    target,
                    dropped_at: at,
                    times,
                    tier: LocationFact::CalendarTimesDropped.problem_tier(),
                });
            }
            _ => {}
        }
    }
    Ok((transaction, problems))
}

/// Each trigger's newest `event_type` event whose trigger is still published in the event's snapshot and whose `open` condition holds, with `$2` bound to a re-check's evaluation or the end of a skip's observation window.
async fn open_problems(
    transaction: ControlTransaction,
    event_type: &str,
    open: &str,
) -> Result<
    (
        ControlTransaction,
        Vec<(PlatformEventEnvelope, PublishedTrigger)>,
    ),
    ControlError,
> {
    let (mut transaction, events) = newest_events(transaction, event_type).await?;
    let mut seen: BTreeSet<Uuid> = BTreeSet::new();
    let mut problems: Vec<(PlatformEventEnvelope, PublishedTrigger)> = Vec::new();
    for envelope in events {
        let (trigger_id, snapshot_id, bound) = match &envelope.payload.platform {
            PlatformEvent::TriggerSkipped {
                trigger_id,
                snapshot_id,
                ..
            } => (
                *trigger_id,
                *snapshot_id,
                Some(Parameter::Time(
                    envelope.occurred_at + TimeDelta::seconds(OBSERVATION_WINDOW_SECONDS),
                )),
            ),
            PlatformEvent::SensorRechecked {
                trigger_id,
                snapshot_id,
                evaluation_id,
            } => (
                *trigger_id,
                *snapshot_id,
                Some(Parameter::Id(*evaluation_id)),
            ),
            PlatformEvent::CalendarTimesDropped {
                trigger_id,
                snapshot_id,
                ..
            } => (*trigger_id, *snapshot_id, None),
            _ => continue,
        };
        if !seen.insert(trigger_id) {
            continue;
        }
        let trigger;
        (transaction, trigger) =
            open_trigger(transaction, (trigger_id, snapshot_id), (open, bound)).await?;
        if let Some(trigger) = trigger {
            problems.push((envelope, trigger));
        }
    }
    Ok((transaction, problems))
}

/// The newest `event_type` platform events, newest first; each trigger's first is the one that counts.
async fn newest_events(
    transaction: ControlTransaction,
    event_type: &str,
) -> Result<(ControlTransaction, Vec<PlatformEventEnvelope>), ControlError> {
    query_records(
        transaction,
        "SELECT storage_id,event_id,occurred_at,payload FROM recall_events WHERE producer='platform' \
         AND event_type=$2 ORDER BY storage_id DESC LIMIT $1",
        vec![
            Parameter::Integer(SKIPPED_TRIGGER_EVENTS),
            Parameter::Text(event_type.to_owned()),
        ],
    )
    .await
}

/// Trigger `trigger_id` while it is still published in `snapshot_id` (as `t`, its snapshot as `p`) and the `open` condition holds, with `$2` bound to the given value.
async fn open_trigger(
    transaction: ControlTransaction,
    (trigger_id, snapshot_id): (Uuid, Uuid),
    (open, bound): (&str, Option<Parameter>),
) -> Result<(ControlTransaction, Option<PublishedTrigger>), ControlError> {
    let (transaction, rows) = query(
        transaction,
        &format!(
            "SELECT t.trigger_id, t.name, t.location_id, l.name AS location_name, t.target, t.snapshot_id \
             FROM sqb_triggers t JOIN sqb_locations l ON l.location_id=t.location_id \
             JOIN sqb_snapshots p ON p.snapshot_id=t.snapshot_id \
             WHERE t.trigger_id=$1 AND t.removed_in_snapshot_id IS NULL AND l.archived_at IS NULL AND {open}"
        ),
        [Parameter::Id(trigger_id)]
            .into_iter()
            .chain(bound)
            .collect(),
    )
    .await?;
    let trigger: Option<PublishedTrigger> = rows.into_iter().next().map(decode_row).transpose()?;
    Ok((
        transaction,
        trigger.filter(|trigger| trigger.snapshot_id == snapshot_id),
    ))
}

/// At most 100 late ticks per target, oldest first.
pub(crate) async fn late_ticks(
    store: &ControlStore,
    targets: impl IntoIterator<Item = &(Uuid, String)>,
    now: DateTime<Utc>,
) -> Result<Vec<OverdueTick>, ControlError> {
    let mut overdue: Vec<OverdueTick> = Vec::new();
    let mut transaction = begin(store, TransactionMode::Read).await?;
    for (location_id, target) in targets {
        let (next, ticks) = query(
            transaction,
            "SELECT t.trigger_id, t.name, t.location_id, l.name AS location_name, t.target, \
             s.next_due_at AS scheduled_for FROM sqb_triggers t \
             JOIN sqb_trigger_state s ON s.trigger_id=t.trigger_id \
             JOIN sqb_locations l ON l.location_id=t.location_id WHERE t.location_id=$1 AND t.target=$2 \
             AND t.kind='schedule' AND s.status='running' AND t.removed_in_snapshot_id IS NULL \
             AND s.next_due_at<$3 ORDER BY s.next_due_at, t.name LIMIT 100",
            vec![
                Parameter::Id(*location_id),
                Parameter::Text(target.clone()),
                Parameter::Time(overdue_before(now)),
            ],
        )
        .await?;
        transaction = next;
        overdue.extend(
            ticks
                .into_iter()
                .map(|row| decode_row(tiered(row, LocationFact::ScheduleOverdue)))
                .collect::<Result<Vec<OverdueTick>, _>>()?,
        );
    }
    commit(transaction).await?;
    Ok(overdue)
}

fn overdue_before(now: DateTime<Utc>) -> DateTime<Utc> {
    now - TimeDelta::seconds(30)
}

/// A calendar is low when its last date is at most a day away; gives when it refreshes and its last date.
pub(crate) fn calendar_low(
    schedule: &Schedule,
    now: DateTime<Utc>,
) -> Option<(DateTime<Utc>, Option<DateTime<Utc>>)> {
    let Schedule::Calendar { times, refresh_at } = schedule else {
        return None;
    };
    let last: Option<DateTime<Utc>> = times.last().copied();
    last.is_none_or(|last| last <= now + TimeDelta::days(1))
        .then_some((*refresh_at, last))
}
