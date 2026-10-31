//! Serialized leases fence results from replaced location instances and uncertain poll retries.

use chrono::Duration;
use uuid::Uuid;

use crate::platform::_helpers::location_work::{read_work, require_instance};
use crate::platform::models::{LocationWorkClaim, LocationWorkPoll};
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};

pub async fn claim_location_work(
    store: &ControlStore,
    location_id: Uuid,
    poll: &LocationWorkPoll,
) -> Result<Option<LocationWorkClaim>, ControlError> {
    let transaction = require_instance(
        begin(store, TransactionMode::Write).await?,
        (location_id, poll.session_id, &poll.target),
    )
    .await?;
    let now = transaction.now;
    let (transaction, rows) = query(transaction,
        "SELECT w.request_id FROM sqb_location_work w WHERE w.location_id=$1 AND w.target=$2 \
         AND w.expires_at>$3 AND NOT EXISTS (SELECT 1 FROM sqb_location_work_results r WHERE r.request_id=w.request_id) \
         AND (w.claimed_until IS NULL OR w.claimed_until<=$3 OR w.session_id=$4) \
         ORDER BY w.requested_at, w.request_id LIMIT 1",
        vec![Parameter::Id(location_id), Parameter::Text(poll.target.clone()), Parameter::Time(now), Parameter::Id(poll.session_id)]).await?;
    let Some(row) = rows.first() else {
        commit(transaction).await?;
        return Ok(None);
    };
    let request_id = serde_json::from_value(row["request_id"].clone())
        .map_err(|error| ControlError::Invalid(format!("invalid work identity: {error}")))?;
    let (transaction, stored) = read_work(transaction, request_id).await?;
    let stored = stored.ok_or_else(|| ControlError::Invalid("location work disappeared".into()))?;
    let retained = stored.session_id == Some(poll.session_id)
        && stored.claimed_until.is_some_and(|until| until > now);
    let (claim_id, lease_expires_at) = if retained {
        (
            stored.claim_id.ok_or(ControlError::Fenced)?,
            stored.claimed_until.ok_or(ControlError::Fenced)?,
        )
    } else {
        (
            Uuid::now_v7(),
            (now + Duration::seconds(60)).min(stored.record.expires_at),
        )
    };
    let (transaction, _) = query(transaction,
        "UPDATE sqb_location_work SET claim_id=$1, session_id=$2, claimed_until=$3 WHERE request_id=$4",
        vec![Parameter::Id(claim_id), Parameter::Id(poll.session_id), Parameter::Time(lease_expires_at), Parameter::Id(request_id)]).await?;
    commit(transaction).await?;
    Ok(Some(LocationWorkClaim {
        request: stored.record.request_json,
        generation: stored.record.generation,
        claim_id,
        session_id: poll.session_id,
        lease_expires_at,
    }))
}
