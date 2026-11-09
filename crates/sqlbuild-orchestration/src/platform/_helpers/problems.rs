//! Live problem queries, with the watchdog's thresholds; callers decide who may see each target's.

use chrono::{DateTime, TimeDelta, Utc};
use uuid::Uuid;

use crate::platform::constants::LOCATION_SILENCE_SECONDS;
use crate::platform::models::{LateTarget, OverdueTick, ProblemScan};
use crate::store::errors::ControlError;
use crate::store::main::decode_row::decode_row;
use crate::store::main::query::query;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

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
    commit(transaction).await?;
    Ok(ProblemScan {
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
