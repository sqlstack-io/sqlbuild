//! Shared fence validation using a fresh database-clock reading in the writer transaction.

use crate::scheduler::models::SchedulerLease;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::main::refresh_clock::refresh_clock;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn check_lease(
    transaction: ControlTransaction,
    lease: &SchedulerLease,
) -> Result<ControlTransaction, ControlError> {
    let transaction = refresh_clock(transaction).await?;
    let now = transaction.now;
    let (transaction, rows) = query(transaction,
        "SELECT epoch FROM sqb_scheduler_lease WHERE name='scheduler' AND holder=$1 AND epoch=$2 AND expires_at>$3",
        vec![Parameter::Id(lease.holder), Parameter::Integer(lease.epoch), Parameter::Time(now)]).await?;
    if rows.is_empty() {
        return Err(ControlError::Fenced);
    }
    Ok(transaction)
}
