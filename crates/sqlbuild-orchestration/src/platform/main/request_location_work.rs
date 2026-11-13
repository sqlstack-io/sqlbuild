//! Named work requires a trusted owner or live person authority; a pending sync covers a repeated one.

use chrono::Duration;
use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::platform::_helpers::connection_tests::authorize_connection_test;
use crate::platform::main::enqueue_location_work::enqueue_location_work;
use crate::platform::main::read_location_work::read_location_work;
use crate::platform::models::{LocationWork, LocationWorkRequest};
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};

pub async fn request_location_work(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Option<Uuid>),
    request: &LocationWorkRequest,
) -> Result<bool, ControlError> {
    match request.work {
        LocationWork::EvaluateSensor { .. } => {
            return Err(ControlError::Invalid(
                "sensor work must originate from a fenced evaluation".into(),
            ));
        }
        LocationWork::Plan { .. } => {
            return Err(ControlError::Invalid(
                "plans must be requested by a person on the current publication".into(),
            ));
        }
        LocationWork::CheckConnection {} | LocationWork::SyncWarehouse {} => {}
    }
    if request.target.trim().is_empty()
        || request.reason.trim().is_empty()
        || context.actor.trim().is_empty()
    {
        return Err(ControlError::Invalid(
            "location work needs a target, reason and trusted actor".into(),
        ));
    }
    let (transaction, context) = authorize_connection_test(
        begin(store, TransactionMode::Write).await?,
        (context, credentials),
        (request.location_id, &request.target),
    )
    .await?;
    let (transaction, previous) = read_location_work(transaction, request.request_id).await?;
    if let Some(previous) = previous {
        if previous.record.request_json != *request || previous.record.requested_by != context.actor
        {
            return Err(ControlError::Invalid(
                "location work ID already has different content or actor".into(),
            ));
        }
        commit(transaction).await?;
        return Ok(false);
    }
    let transaction = if matches!(request.work, LocationWork::SyncWarehouse {}) {
        let now = transaction.now;
        let (transaction, pending) = query(transaction,
            "SELECT request_id FROM sqb_location_work WHERE location_id=$1 AND target=$2 \
             AND completed_at IS NULL AND expires_at>$3 AND request_json->'work'->>'kind'='sync_warehouse' LIMIT 1",
            vec![Parameter::Id(request.location_id), Parameter::Text(request.target.clone()), Parameter::Time(now)]).await?;
        if !pending.is_empty() {
            commit(transaction).await?;
            return Ok(false);
        }
        transaction
    } else {
        transaction
    };
    commit(enqueue_location_work(transaction, &context, request, Duration::minutes(5)).await?)
        .await?;
    Ok(true)
}
