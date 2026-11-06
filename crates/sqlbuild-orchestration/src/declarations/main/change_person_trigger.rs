//! A trigger override needs its permission on every node the trigger's current job or group writes.

use serde_json::json;
use uuid::Uuid;

use crate::authorization::main::authorize_person::authorize_person;
use crate::authorization::main::deny_authorization::deny;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::declarations::_helpers::operator_change::change_trigger_in;
use crate::declarations::_helpers::trigger_scope::trigger_scope;
use crate::declarations::models::TriggerChangeRequest;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

pub async fn change_person_trigger(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Uuid),
    request: &TriggerChangeRequest,
) -> Result<bool, ControlError> {
    let (transaction, scope) = trigger_scope(
        begin(store, TransactionMode::Write).await?,
        (context, credentials),
        request,
    )
    .await?;
    let (transaction, context) = authorize_person(
        transaction,
        (context, credentials),
        &scope,
        DefaultRole::None,
    )
    .await?;
    if request.reason.trim().is_empty() {
        return deny(
            transaction,
            &context,
            json!({"trigger_id":request.trigger_id}),
            "trigger override requires a reason",
        )
        .await;
    }
    let request = TriggerChangeRequest {
        actor: context.actor.clone(),
        ..request.clone()
    };
    let (transaction, applied) = change_trigger_in(transaction, &context, &request).await?;
    commit(transaction).await?;
    Ok(applied)
}
