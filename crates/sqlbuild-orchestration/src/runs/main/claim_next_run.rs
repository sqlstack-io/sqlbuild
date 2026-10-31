//! Claim priority-ordered work without allowing a blocked page to starve later candidates.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU32;

use sqlbuild_recall::schema::models::ColumnType;

use crate::runs::_helpers::groups::admission::{earlier_waiter_blocks, group_capacity};
use crate::runs::_helpers::groups::selection::child_ready;
use crate::runs::_helpers::lifecycle::claims::{publish_claim, reserve_claims};
use crate::runs::models::{ConcurrencyResource, QueueCandidate, RunClaim, RunDispatch};
use crate::scheduler::main::check_lease::check_lease;
use crate::scheduler::models::SchedulerLease;
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::decode_row::decode_row;
use crate::store::main::query::query;
use crate::store::models::{Connection, ControlStore, Parameter, TransactionMode};

/// Limits come from one installation configuration, never from competing per-job declarations.
pub async fn claim_next_run<'a>(
    store: &ControlStore,
    lease: &SchedulerLease,
    limits: &BTreeMap<String, NonZeroU32>,
    dispatch: impl Into<RunDispatch<'a>>,
) -> Result<Option<RunClaim>, ControlError> {
    let dispatch = dispatch.into();
    let (mut transaction, capacity) = group_capacity(
        check_lease(begin(store, TransactionMode::Write).await?, lease).await?,
        limits,
        dispatch.eligible,
    )
    .await?;
    let mut waiting: BTreeSet<ConcurrencyResource> = BTreeSet::new();
    let locking = if matches!(transaction.connection, Some(Connection::Postgres(_))) {
        " FOR UPDATE OF r SKIP LOCKED"
    } else {
        ""
    };
    let mut after: Option<QueueCandidate> = None;
    loop {
        let rows;
        (transaction, rows) = query(transaction,
            &format!("SELECT r.request_json, r.queued_at FROM sqb_runs r WHERE r.status='queued' AND r.kind<>'group' AND r.cancel_requested_at IS NULL \
             AND NOT EXISTS (SELECT 1 FROM sqb_run_liveness l WHERE l.run_id=r.run_id) \
             AND ($4 IS NULL OR (r.location_id=$4 AND r.target=$5)) \
             AND ($1 IS NULL OR r.priority<$1 OR (r.priority=$1 AND (r.queued_at>$2 OR (r.queued_at=$2 AND r.run_id>$3)))) \
             ORDER BY r.priority DESC, r.queued_at, r.run_id LIMIT 100{locking}"),
            vec![after.as_ref().map_or(Parameter::Null(ColumnType::Integer), |value| Parameter::Integer(value.request_json.priority)),
                Parameter::OptionalTime(after.as_ref().map(|value| value.queued_at)),
                Parameter::OptionalId(after.as_ref().map(|value| value.request_json.run_id)),
                Parameter::OptionalId(dispatch.scope.map(|(location, _)| location)),
                Parameter::OptionalText(dispatch.scope.map(|(_, target)| target.to_owned()))]).await?;
        if rows.is_empty() {
            commit(check_lease(transaction, lease).await?).await?;
            return Ok(None);
        }
        for row in rows {
            let candidate: QueueCandidate = decode_row(row)?;
            let ready;
            (transaction, ready) = child_ready(transaction, &candidate.request_json).await?;
            if !ready {
                after = Some(candidate);
                continue;
            }
            let blocked;
            (transaction, blocked) =
                earlier_waiter_blocks(transaction, &capacity, &candidate).await?;
            if blocked || capacity.blocks_claim(&candidate, &waiting) {
                after = Some(candidate);
                continue;
            }
            (transaction, _) = query(transaction, "SAVEPOINT candidate_claim", vec![]).await?;
            let reserved;
            (transaction, reserved) =
                reserve_claims(transaction, &candidate.request_json, limits).await?;
            if !reserved {
                (transaction, _) =
                    query(transaction, "ROLLBACK TO SAVEPOINT candidate_claim", vec![]).await?;
                (transaction, _) =
                    query(transaction, "RELEASE SAVEPOINT candidate_claim", vec![]).await?;
                waiting = capacity.record_waiter(&candidate.request_json, waiting);
                after = Some(candidate);
                continue;
            }
            let claim;
            (transaction, claim) = publish_claim(transaction, candidate.request_json).await?;
            (transaction, _) =
                query(transaction, "RELEASE SAVEPOINT candidate_claim", vec![]).await?;
            commit(check_lease(transaction, lease).await?).await?;
            return Ok(Some(claim));
        }
    }
}
