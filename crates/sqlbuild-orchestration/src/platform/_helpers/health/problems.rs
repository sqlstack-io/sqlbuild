//! Live problem queries, with the watchdog's thresholds; callers decide who may see each target's.

use chrono::{DateTime, TimeDelta, Utc};
use uuid::Uuid;

use crate::declarations::models::Schedule;
use crate::platform::constants::LOCATION_SILENCE_SECONDS;
use std::collections::BTreeSet;

use crate::platform::constants::SKIPPED_TRIGGER_EVENTS;
use crate::platform::models::{
    LateTarget, LowCalendar, OverdueTick, PlatformEvent, PlatformEventEnvelope, ProblemScan,
    PublishedTrigger, RunningSchedule, SkippedTrigger,
};
use crate::store::errors::ControlError;
use crate::store::main::decode_row::decode_row;
use crate::store::main::query::query;
use crate::store::main::query_records::query_records;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, ControlTransaction, Parameter, TransactionMode};

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
    let (transaction, skipped_triggers) = skipped_triggers(transaction).await?;
    commit(transaction).await?;
    let schedules: Vec<RunningSchedule> = schedules
        .into_iter()
        .map(decode_row)
        .collect::<Result<_, _>>()?;
    let low_calendars: Vec<LowCalendar> = schedules
        .into_iter()
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
            })
        })
        .collect();
    Ok(ProblemScan {
        low_calendars,
        skipped_triggers,
        now,
        lease: leases.into_iter().next().map(decode_row).transpose()?,
        broken: locations
            .into_iter()
            .map(decode_row)
            .collect::<Result<_, _>>()?,
        late_targets: late_targets
            .into_iter()
            .map(|row| {
                let target: LateTarget = decode_row(row)?;
                Ok((target.location_id, target.target))
            })
            .collect::<Result<_, ControlError>>()?,
    })
}

/// Triggers whose newest `trigger_skipped` event names the snapshot they are still published in, and that have not ticked successfully since it was published: their location has not published again, and no tick has run since.
async fn skipped_triggers(
    transaction: ControlTransaction,
) -> Result<(ControlTransaction, Vec<SkippedTrigger>), ControlError> {
    let (mut transaction, events): (ControlTransaction, Vec<PlatformEventEnvelope>) =
        query_records(
            transaction,
            "SELECT storage_id,event_id,occurred_at,payload FROM recall_events WHERE producer='platform' \
             AND event_type='platform_trigger_skipped' ORDER BY storage_id DESC LIMIT $1",
            vec![Parameter::Integer(SKIPPED_TRIGGER_EVENTS)],
        )
        .await?;
    let mut seen: BTreeSet<Uuid> = BTreeSet::new();
    let mut skipped: Vec<SkippedTrigger> = Vec::new();
    for envelope in events {
        let PlatformEvent::TriggerSkipped {
            trigger_id,
            snapshot_id,
            reason,
        } = envelope.payload.platform
        else {
            continue;
        };
        if !seen.insert(trigger_id) {
            continue;
        }
        let rows;
        (transaction, rows) = query(
            transaction,
            "SELECT t.name, t.location_id, l.name AS location_name, t.target, t.snapshot_id \
             FROM sqb_triggers t JOIN sqb_locations l ON l.location_id=t.location_id \
             JOIN sqb_snapshots p ON p.snapshot_id=t.snapshot_id \
             WHERE t.trigger_id=$1 AND t.removed_in_snapshot_id IS NULL AND l.archived_at IS NULL \
             AND NOT EXISTS (SELECT 1 FROM sqb_ticks k WHERE k.trigger_id=t.trigger_id \
             AND k.outcome IN ('requested','covered') AND k.evaluated_at>=p.published_at)",
            vec![Parameter::Id(trigger_id)],
        )
        .await?;
        let Some(row) = rows.into_iter().next() else {
            continue;
        };
        let trigger: PublishedTrigger = decode_row(row)?;
        if trigger.snapshot_id == snapshot_id {
            skipped.push(SkippedTrigger {
                trigger_id,
                name: trigger.name,
                location_id: trigger.location_id,
                location_name: trigger.location_name,
                target: trigger.target,
                skipped_at: envelope.occurred_at,
                reason,
            });
        }
    }
    Ok((transaction, skipped))
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
                .map(decode_row)
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
