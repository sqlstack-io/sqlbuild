//! A person asks a location to compile and publish again; the request names them for the location.

use chrono::Duration;
use uuid::Uuid;

use crate::authorization::main::decisions::authorize_person::authorize_person;
use crate::credentials::models::{Action, AuditContext, DefaultRole, PermissionScope};
use crate::platform::constants::REPUBLISH_EXPIRY_SECONDS;
use crate::platform::main::work::enqueue_location_work::enqueue_location_work;
use crate::platform::main::work::read_location_work::read_location_work;
use crate::platform::models::{LocationWork, LocationWorkRequest, PersonWorkRequest};
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// Queue a republish of `target`; `false` when this request ID was already queued by the same caller.
pub async fn request_republish(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Option<Uuid>),
    (location_id, target): (Uuid, &str),
    request: &PersonWorkRequest,
) -> Result<bool, ControlError> {
    if target.trim().is_empty() || request.reason.trim().is_empty() {
        return Err(ControlError::Invalid(
            "a republish needs a target and a reason".into(),
        ));
    }
    let transaction = begin(store, TransactionMode::Write).await?;
    let (transaction, context, requested_by) = match credentials {
        Some(principal_id) => {
            let (transaction, context) = authorize_person(
                transaction,
                (context, principal_id),
                &PermissionScope::action(Action::LocationRepublish, Some((location_id, target))),
                DefaultRole::None,
            )
            .await?;
            let (transaction, rows) = query(
                transaction,
                "SELECT name FROM sqb_principals WHERE principal_id=$1",
                vec![Parameter::Id(principal_id)],
            )
            .await?;
            let name: String = rows
                .first()
                .and_then(|row| row["name"].as_str())
                .map_or_else(|| context.actor.clone(), str::to_owned);
            (transaction, context, name)
        }
        None => (transaction, context.clone(), context.actor.clone()),
    };
    let work = LocationWorkRequest {
        request_id: request.request_id,
        location_id,
        target: target.to_owned(),
        reason: request.reason.clone(),
        work: LocationWork::Republish { requested_by },
    };
    let (transaction, previous) = read_location_work(transaction, request.request_id).await?;
    if let Some(previous) = previous {
        if previous.record.request_json != work || previous.record.requested_by != context.actor {
            return Err(ControlError::Invalid(
                "location work ID already has different content or actor".into(),
            ));
        }
        commit(transaction).await?;
        return Ok(false);
    }
    commit(
        enqueue_location_work(
            transaction,
            &context,
            &work,
            Duration::seconds(REPUBLISH_EXPIRY_SECONDS),
        )
        .await?,
    )
    .await?;
    Ok(true)
}
