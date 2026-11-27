//! Anyone who can see a target's runs dismisses the problem of a stored event skipped as unreadable, for good, as an audit event; the skip stays in the history.

use serde_json::json;
use uuid::Uuid;

use crate::acknowledgements::constants::SKIP_DISMISSED;
use crate::acknowledgements::models::SkippedEventDismissalRequest;
use crate::authorization::main::decisions::authorize_listing::authorize_listing;
use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::platform::main::health::recorded_skip::recorded_skip;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{Change, ControlStore, Parameter, TransactionMode};

/// Whether the dismissal changed anything: a skip of this target, or a server-wide one, is dismissed once; a retry or a second dismissal applies nothing.
pub async fn dismiss_person_skipped_event(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Uuid),
    (location_id, target, request): (Uuid, &str, &SkippedEventDismissalRequest),
    default_role: DefaultRole,
) -> Result<bool, ControlError> {
    let (transaction, context) = authorize_listing(
        begin(store, TransactionMode::Write).await?,
        (context, credentials),
        (location_id, target),
        default_role,
    )
    .await?;
    let body =
        serde_json::to_value(request).map_err(|error| ControlError::Invalid(error.to_string()))?;
    let (transaction, retried) = query(
        transaction,
        "SELECT scope_json FROM sqb_security_audit WHERE event_id=$1",
        vec![Parameter::Id(request.event_id)],
    )
    .await?;
    if let Some(previous) = retried.first() {
        if previous["scope_json"].get("request") != Some(&body) {
            return Err(ControlError::Invalid(
                "dismissal event ID already has different content".into(),
            ));
        }
        commit(transaction).await?;
        return Ok(false);
    }
    let (transaction, skip, dismissed) = recorded_skip(transaction, request.storage_id).await?;
    let Some(skip) = skip else {
        return Err(ControlError::Invalid(format!(
            "no skipped stored event with storage ID {}",
            request.storage_id
        )));
    };
    if skip
        .location_id
        .zip(skip.target.clone())
        .is_some_and(|scope| scope != (location_id, target.to_owned()))
    {
        return Err(ControlError::Invalid(
            "the skipped stored event belongs to another location target".into(),
        ));
    }
    if dismissed {
        commit(transaction).await?;
        return Ok(false);
    }
    let mut transaction = append_audit(
        transaction,
        &context,
        (request.event_id, SKIP_DISMISSED),
        (
            json!({"location_id": location_id, "target": target, "storage_id": request.storage_id,
                "skipped_event_id": skip.event_id, "event_type": skip.event_type, "request": body}),
            json!({"dismissed": false}),
            json!({"dismissed": true}),
        ),
    )
    .await?;
    transaction
        .changes
        .insert(Change::Attention(location_id, target.into()));
    commit(transaction).await?;
    Ok(true)
}
