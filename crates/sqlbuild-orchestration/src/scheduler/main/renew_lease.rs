//! A paused or superseded scheduler cannot renew an expired fence.

use chrono::TimeDelta;

use crate::scheduler::constants::LEASE_TTL_SECONDS;
use crate::scheduler::main::check_lease::check_lease;
use crate::scheduler::models::SchedulerLease;
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// Renewal preserves the epoch; only takeover allocates a new epoch.
pub async fn renew_lease(
    store: &ControlStore,
    lease: &SchedulerLease,
) -> Result<SchedulerLease, ControlError> {
    let transaction = begin(store, TransactionMode::Write).await?;
    let transaction = check_lease(transaction, lease).await?;
    let expires_at = transaction.now + TimeDelta::seconds(LEASE_TTL_SECONDS);
    let (transaction, _) = query(transaction,
        "UPDATE sqb_scheduler_lease SET expires_at=$1 WHERE name='scheduler' AND holder=$2 AND epoch=$3",
        vec![Parameter::Time(expires_at), Parameter::Id(lease.holder), Parameter::Integer(lease.epoch)]).await?;
    commit(transaction).await?;
    Ok(SchedulerLease {
        expires_at,
        ..lease.clone()
    })
}
