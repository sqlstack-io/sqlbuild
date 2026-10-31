//! Elect one scheduler under the control store's serialized writer lock.

use chrono::TimeDelta;
use uuid::Uuid;

use crate::scheduler::constants::LEASE_TTL_SECONDS;
use crate::scheduler::models::SchedulerLease;
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// An existing live lease is never stolen, including by a duplicate caller with the same holder.
pub async fn acquire_lease(
    store: &ControlStore,
    holder: Uuid,
) -> Result<Option<SchedulerLease>, ControlError> {
    let transaction = begin(store, TransactionMode::Write).await?;
    let now = transaction.now;
    let expires_at = now + TimeDelta::seconds(LEASE_TTL_SECONDS);
    let (transaction, rows) = query(transaction,
        "INSERT INTO sqb_scheduler_lease (name, holder, epoch, expires_at) VALUES ('scheduler',$1,1,$2) \
         ON CONFLICT (name) DO UPDATE SET holder=excluded.holder, epoch=sqb_scheduler_lease.epoch+1, expires_at=excluded.expires_at \
         WHERE sqb_scheduler_lease.expires_at<=$3 RETURNING epoch",
        vec![Parameter::Id(holder), Parameter::Time(expires_at), Parameter::Time(now)]).await?;
    let lease = rows
        .first()
        .and_then(|row| row["epoch"].as_i64())
        .map(|epoch| SchedulerLease {
            holder,
            epoch,
            expires_at,
        });
    commit(transaction).await?;
    Ok(lease)
}
