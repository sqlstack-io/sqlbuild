//! Resume virtual parents independently of launcher capacity or process restarts.

use std::collections::BTreeMap;
use std::num::NonZeroU32;
use std::time::{Duration, Instant};

use crate::runs::_helpers::groups::lifecycle::advance_group;
use crate::runs::models::RunDispatch;
use crate::scheduler::constants::RENEW_INTERVAL_SECONDS;
use crate::scheduler::main::{check_lease::check_lease, renew_lease::renew_lease};
use crate::scheduler::models::SchedulerLease;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

pub async fn advance_groups<'a>(
    store: &ControlStore,
    lease: &SchedulerLease,
    (limits, dispatch): (&BTreeMap<String, NonZeroU32>, impl Into<RunDispatch<'a>>),
) -> Result<(), ControlError> {
    let dispatch = dispatch.into();
    let scope = dispatch.scope.ok_or_else(|| {
        ControlError::Invalid("group advancement requires a dispatch scope".into())
    })?;
    let mut after = None;
    let mut renewed = Instant::now();
    loop {
        let transaction = begin(store, TransactionMode::Read).await?;
        let (transaction, rows) = query(transaction,
            "SELECT run_id FROM sqb_runs WHERE kind='group' AND status IN ('queued','running') \
             AND location_id=$1 AND target=$2 AND ($3 IS NULL OR run_id>$3) ORDER BY run_id LIMIT 100",
            vec![Parameter::Id(scope.0), Parameter::Text(scope.1.into()), Parameter::OptionalId(after)]).await?;
        commit(transaction).await?;
        if rows.is_empty() {
            return Ok(());
        }
        for row in rows {
            if renewed.elapsed() >= Duration::from_secs(RENEW_INTERVAL_SECONDS) {
                renew_lease(store, lease).await?;
                renewed = Instant::now();
            }
            let run_id: uuid::Uuid = serde_json::from_value(row["run_id"].clone())
                .map_err(|error| ControlError::Invalid(error.to_string()))?;
            let transaction =
                check_lease(begin(store, TransactionMode::Write).await?, lease).await?;
            let transaction = advance_group(transaction, run_id, limits, dispatch.eligible).await?;
            commit(check_lease(transaction, lease).await?).await?;
            after = Some(run_id);
        }
    }
}
