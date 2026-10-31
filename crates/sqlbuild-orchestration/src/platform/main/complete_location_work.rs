//! A completion is immutable; only the current unexpired claim may publish its result.

use serde_json::json;
use uuid::Uuid;

use crate::platform::_helpers::location_work::{read_work, require_instance};
use crate::platform::models::LocationWorkResult;
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};

pub async fn complete_location_work(
    store: &ControlStore,
    (location_id, request_id): (Uuid, Uuid),
    result: &LocationWorkResult,
) -> Result<bool, ControlError> {
    let transaction = begin(store, TransactionMode::Write).await?;
    let (transaction, stored) = read_work(transaction, request_id).await?;
    let stored = stored
        .filter(|stored| stored.record.request_json.location_id == location_id)
        .ok_or_else(|| ControlError::Invalid("work is not assigned to this location".into()))?;
    let transaction = require_instance(
        transaction,
        (
            location_id,
            result.session_id,
            &stored.record.request_json.target,
        ),
    )
    .await?;
    if let Some(previous) = stored.record.result_json {
        if previous != *result {
            return Err(ControlError::Invalid(
                "location work already has a different result".into(),
            ));
        }
        commit(transaction).await?;
        return Ok(false);
    }
    if stored.claim_id != Some(result.claim_id)
        || stored.session_id != Some(result.session_id)
        || stored
            .claimed_until
            .is_none_or(|until| until <= transaction.now)
        || stored.record.expires_at <= transaction.now
    {
        return Err(ControlError::Fenced);
    }
    let now = transaction.now;
    let (transaction, _) = query(transaction,
        "INSERT INTO sqb_location_work_results (request_id,result_json,completed_at) VALUES ($1,$2,$3)",
        vec![Parameter::Id(request_id), Parameter::Json(json!(result)), Parameter::Time(now)]).await?;
    commit(transaction).await?;
    Ok(true)
}
