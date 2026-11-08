//! People see the scheduler's state, and broken locations and late ticks only for targets they list.

use std::collections::BTreeSet;

use chrono::TimeDelta;
use uuid::Uuid;

use crate::authorization::main::person_lists_target::person_lists_target;
use crate::credentials::models::DefaultRole;
use crate::platform::models::{BrokenLocation, LateTarget, OverdueTick, PlatformProblems};
use crate::store::errors::ControlError;
use crate::store::main::decode_row::decode_row;
use crate::store::main::query::query;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// Uses the watchdog's thresholds; access is checked per target before each target's ticks are read.
pub async fn read_person_problems(
    store: &ControlStore,
    principal_id: Uuid,
    default_role: DefaultRole,
) -> Result<PlatformProblems, ControlError> {
    let transaction = begin(store, TransactionMode::Read).await?;
    let now = transaction.now;
    let overdue_before = now - TimeDelta::seconds(30);
    let (transaction, leases) = query(
        transaction,
        "SELECT name FROM sqb_scheduler_lease WHERE name='scheduler' AND expires_at>$1",
        vec![Parameter::Time(now)],
    )
    .await?;
    let (transaction, locations) = query(
        transaction,
        "SELECT i.location_id, i.target, MAX(i.last_seen_at) AS last_seen_at \
         FROM sqb_location_instances i JOIN sqb_locations l ON l.location_id=i.location_id \
         WHERE l.archived_at IS NULL GROUP BY i.location_id,i.target \
         HAVING MAX(i.last_seen_at)<$1 ORDER BY i.location_id,i.target",
        vec![Parameter::Time(now - TimeDelta::seconds(90))],
    )
    .await?;
    let (transaction, late_targets) = query(
        transaction,
        "SELECT DISTINCT t.location_id, t.target FROM sqb_triggers t \
         JOIN sqb_trigger_state s ON s.trigger_id=t.trigger_id \
         JOIN sqb_locations l ON l.location_id=t.location_id \
         WHERE t.kind='schedule' AND s.status='running' AND t.removed_in_snapshot_id IS NULL \
         AND l.archived_at IS NULL AND s.next_due_at<$1",
        vec![Parameter::Time(overdue_before)],
    )
    .await?;
    commit(transaction).await?;
    let locations: Vec<BrokenLocation> = locations
        .into_iter()
        .map(decode_row)
        .collect::<Result<_, _>>()?;
    let late_targets: BTreeSet<(Uuid, String)> = late_targets
        .into_iter()
        .map(|row| {
            let target: LateTarget = decode_row(row)?;
            Ok((target.location_id, target.target))
        })
        .collect::<Result<_, ControlError>>()?;
    let mut listed: BTreeSet<(Uuid, String)> = BTreeSet::new();
    for (location_id, target) in locations
        .iter()
        .map(|location| (location.location_id, location.target.clone()))
        .chain(late_targets.iter().cloned())
        .collect::<BTreeSet<_>>()
    {
        if person_lists_target(store, principal_id, (location_id, &target), default_role).await? {
            listed.insert((location_id, target));
        }
    }
    let mut overdue_ticks: Vec<OverdueTick> = Vec::new();
    let mut transaction = begin(store, TransactionMode::Read).await?;
    for (location_id, target) in late_targets.intersection(&listed) {
        let (next, ticks) = query(
            transaction,
            "SELECT t.trigger_id, t.name, t.location_id, t.target, s.next_due_at AS scheduled_for \
             FROM sqb_triggers t JOIN sqb_trigger_state s ON s.trigger_id=t.trigger_id \
             WHERE t.location_id=$1 AND t.target=$2 AND t.kind='schedule' AND s.status='running' \
             AND t.removed_in_snapshot_id IS NULL AND s.next_due_at<$3 \
             ORDER BY s.next_due_at, t.name LIMIT 100",
            vec![
                Parameter::Id(*location_id),
                Parameter::Text(target.clone()),
                Parameter::Time(overdue_before),
            ],
        )
        .await?;
        transaction = next;
        overdue_ticks.extend(
            ticks
                .into_iter()
                .map(decode_row)
                .collect::<Result<Vec<OverdueTick>, _>>()?,
        );
    }
    commit(transaction).await?;
    Ok(PlatformProblems {
        scheduler_leader: !leases.is_empty(),
        broken_locations: locations
            .into_iter()
            .filter(|location| listed.contains(&(location.location_id, location.target.clone())))
            .collect(),
        overdue_ticks,
    })
}
