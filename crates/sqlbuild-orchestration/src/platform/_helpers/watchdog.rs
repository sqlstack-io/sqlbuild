//! Health scanning releases the writer lock between bounded trigger pages.

use chrono::TimeDelta;
use tokio::sync::watch;
use uuid::Uuid;

use crate::declarations::models::Schedule;
use crate::platform::_helpers::observations::observe;
use crate::platform::models::{InstallationHealth, PlatformEvent, WatchdogTrigger};
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::decode_row::decode_row;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};

pub(crate) async fn scan_health(
    store: &ControlStore,
    replica_id: Uuid,
    shutdown: &watch::Receiver<bool>,
) -> Result<(), ControlError> {
    let transaction = begin(store, TransactionMode::Write).await?;
    let now = transaction.now;
    let (transaction, rows) = query(transaction,
        "SELECT i.installation_id, CASE WHEN l.expires_at>$1 THEN 1 ELSE 0 END AS leader \
         FROM sqb_installation i LEFT JOIN sqb_scheduler_lease l ON l.name='scheduler' WHERE i.singleton=1",
        vec![Parameter::Time(now)]).await?;
    let health: InstallationHealth = decode_row(
        rows.into_iter()
            .next()
            .ok_or_else(|| ControlError::Invalid("control store has no installation".into()))?,
    )?;
    let mut transaction = transaction;
    if health.leader == 0 {
        transaction = observe(
            transaction,
            health.installation_id,
            PlatformEvent::SchedulerNoLeader,
        )
        .await?;
    }
    commit(transaction).await?;
    let mut after = None;
    while !*shutdown.borrow() {
        after = scan_trigger_page(store, health.installation_id, after).await?;
        if after.is_none() {
            break;
        }
        tokio::task::yield_now().await;
    }
    if *shutdown.borrow() {
        return Ok(());
    }
    let transaction = begin(store, TransactionMode::Write).await?;
    commit(
        observe(
            transaction,
            health.installation_id,
            PlatformEvent::Heartbeat { replica_id },
        )
        .await?,
    )
    .await?;
    Ok(())
}

async fn scan_trigger_page(
    store: &ControlStore,
    installation_id: Uuid,
    after: Option<Uuid>,
) -> Result<Option<Uuid>, ControlError> {
    let cursor = after.map_or("", |_| "AND t.trigger_id>$1");
    let transaction = begin(store, TransactionMode::Write).await?;
    let now = transaction.now;
    let (mut transaction, rows) = query(
        transaction,
        &format!(
            "SELECT t.trigger_id, t.definition_json, s.next_due_at \
         FROM sqb_triggers t JOIN sqb_trigger_state s ON s.trigger_id=t.trigger_id \
         JOIN sqb_locations l ON l.location_id=t.location_id \
         WHERE s.status='running' AND t.removed_in_snapshot_id IS NULL AND l.archived_at IS NULL \
         {cursor} ORDER BY t.trigger_id LIMIT 100"
        ),
        after.into_iter().map(Parameter::Id).collect(),
    )
    .await?;
    let triggers: Vec<WatchdogTrigger> =
        rows.into_iter().map(decode_row).collect::<Result<_, _>>()?;
    let next = triggers.last().map(|trigger| trigger.trigger_id);
    for trigger in triggers {
        if let Some(scheduled_for) = trigger.next_due_at
            && scheduled_for < now - TimeDelta::seconds(30)
        {
            transaction = observe(
                transaction,
                installation_id,
                PlatformEvent::TickOverdue {
                    trigger_id: trigger.trigger_id,
                    scheduled_for,
                },
            )
            .await?;
        }
        if let Schedule::Calendar { times, refresh_at } = trigger.definition_json.schedule
            && times
                .last()
                .is_none_or(|last| *last <= now + TimeDelta::days(1))
        {
            transaction = observe(
                transaction,
                installation_id,
                PlatformEvent::CalendarHorizonLow {
                    trigger_id: trigger.trigger_id,
                    refresh_at,
                    horizon_end: times.last().copied(),
                },
            )
            .await?;
        }
    }
    commit(transaction).await?;
    Ok(next)
}
