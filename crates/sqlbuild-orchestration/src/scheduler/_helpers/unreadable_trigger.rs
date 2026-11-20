//! Skip one trigger whose stored job or snapshot can't be read, so every other trigger keeps scheduling.

use chrono::TimeDelta;
use uuid::Uuid;

use crate::platform::main::health::record_trigger_problem::record_trigger_problem;
use crate::platform::models::PlatformEvent;
use crate::scheduler::constants::{UNREADABLE_TRIGGER, UNREADABLE_TRIGGER_RETRY_SECONDS};
use crate::scheduler::main::check_lease::check_lease;
use crate::scheduler::models::SchedulerLease;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// Evaluate `trigger_id` again later, and record one `trigger_skipped` health event per trigger and snapshot.
pub(crate) async fn skip_unreadable_trigger(
    store: &ControlStore,
    lease: &SchedulerLease,
    trigger_id: Uuid,
) -> Result<(), ControlError> {
    let transaction = check_lease(begin(store, TransactionMode::Write).await?, lease).await?;
    let retry_at = transaction.now + TimeDelta::seconds(UNREADABLE_TRIGGER_RETRY_SECONDS);
    let (transaction, _) = query(
        transaction,
        "UPDATE sqb_trigger_state SET next_due_at=$2 WHERE trigger_id=$1",
        vec![Parameter::Id(trigger_id), Parameter::Time(retry_at)],
    )
    .await?;
    let (transaction, rows) = query(
        transaction,
        "SELECT snapshot_id FROM sqb_triggers WHERE trigger_id=$1",
        vec![Parameter::Id(trigger_id)],
    )
    .await?;
    let Some(row) = rows.into_iter().next() else {
        return commit(check_lease(transaction, lease).await?).await;
    };
    let snapshot_id = serde_json::from_value::<Uuid>(row["snapshot_id"].clone())
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    let transaction = record_trigger_problem(
        transaction,
        PlatformEvent::TriggerSkipped {
            trigger_id,
            snapshot_id,
            reason: UNREADABLE_TRIGGER.to_owned(),
        },
    )
    .await?;
    commit(check_lease(transaction, lease).await?).await
}
