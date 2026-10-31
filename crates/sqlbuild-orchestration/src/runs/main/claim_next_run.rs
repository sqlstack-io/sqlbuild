//! Claim priority-ordered work without allowing a blocked page to starve later candidates.

use std::collections::BTreeMap;
use std::num::NonZeroU32;

use serde_json::json;
use sqlbuild_recall::schema::models::ColumnType;
use uuid::Uuid;

use crate::runs::_helpers::claims::reserve_claims;
use crate::runs::_helpers::events::append_run_event;
use crate::runs::models::{QueueCandidate, RunClaim};
use crate::scheduler::main::check_lease::check_lease;
use crate::scheduler::models::SchedulerLease;
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::decode_row::decode_row;
use crate::store::main::query::query;
use crate::store::models::{Connection, ControlStore, Parameter, TransactionMode};

/// Limits come from one installation configuration, never from competing per-job declarations.
pub async fn claim_next_run(
    store: &ControlStore,
    lease: &SchedulerLease,
    limits: &BTreeMap<String, NonZeroU32>,
    scope: Option<(Uuid, &str)>,
) -> Result<Option<RunClaim>, ControlError> {
    let mut transaction = check_lease(begin(store, TransactionMode::Write).await?, lease).await?;
    let locking = if matches!(transaction.connection, Some(Connection::Postgres(_))) {
        " FOR UPDATE OF r SKIP LOCKED"
    } else {
        ""
    };
    let mut after: Option<QueueCandidate> = None;
    loop {
        let rows;
        (transaction, rows) = query(transaction,
            &format!("SELECT r.request_json, r.queued_at FROM sqb_runs r WHERE r.status='queued' AND r.cancel_requested_at IS NULL \
             AND NOT EXISTS (SELECT 1 FROM sqb_run_liveness l WHERE l.run_id=r.run_id) \
             AND ($4 IS NULL OR (r.location_id=$4 AND r.target=$5)) \
             AND ($1 IS NULL OR r.priority<$1 OR (r.priority=$1 AND (r.queued_at>$2 OR (r.queued_at=$2 AND r.run_id>$3)))) \
             ORDER BY r.priority DESC, r.queued_at, r.run_id LIMIT 100{locking}"),
            vec![after.as_ref().map_or(Parameter::Null(ColumnType::Integer), |value| Parameter::Integer(value.request_json.priority)),
                Parameter::OptionalTime(after.as_ref().map(|value| value.queued_at)),
                Parameter::OptionalId(after.as_ref().map(|value| value.request_json.run_id)),
                Parameter::OptionalId(scope.map(|(location, _)| location)),
                Parameter::OptionalText(scope.map(|(_, target)| target.to_owned()))]).await?;
        if rows.is_empty() {
            commit(check_lease(transaction, lease).await?).await?;
            return Ok(None);
        }
        for row in rows {
            let candidate: QueueCandidate = decode_row(row)?;
            (transaction, _) = query(transaction, "SAVEPOINT candidate_claim", vec![]).await?;
            let reserved;
            (transaction, reserved) =
                reserve_claims(transaction, &candidate.request_json, limits).await?;
            if !reserved {
                (transaction, _) =
                    query(transaction, "ROLLBACK TO SAVEPOINT candidate_claim", vec![]).await?;
                (transaction, _) =
                    query(transaction, "RELEASE SAVEPOINT candidate_claim", vec![]).await?;
                after = Some(candidate);
                continue;
            }
            let claim = RunClaim {
                claim_id: Uuid::new_v5(&candidate.request_json.run_id, b"run_claimed"),
                request: candidate.request_json,
            };
            let now = transaction.now;
            (transaction, _) = query(transaction,
                "INSERT INTO sqb_run_liveness (run_id, worker_instance, claimed_at, last_heartbeat_at) VALUES ($1,$2,$3,$3)",
                vec![Parameter::Id(claim.request.run_id), Parameter::Id(claim.claim_id), Parameter::Time(now)]).await?;
            let cursor;
            (transaction, cursor) = append_run_event(
                transaction,
                (claim.request.run_id, claim.claim_id),
                "run_claimed",
                json!({"claim": claim}),
            )
            .await?;
            (transaction, _) = query(
                transaction,
                "UPDATE sqb_runs SET last_event_storage_id=$1 WHERE run_id=$2",
                vec![
                    Parameter::Integer(cursor),
                    Parameter::Id(claim.request.run_id),
                ],
            )
            .await?;
            (transaction, _) =
                query(transaction, "RELEASE SAVEPOINT candidate_claim", vec![]).await?;
            commit(check_lease(transaction, lease).await?).await?;
            return Ok(Some(claim));
        }
    }
}
