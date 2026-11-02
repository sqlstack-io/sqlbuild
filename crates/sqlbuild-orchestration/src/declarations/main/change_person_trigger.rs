//! Resolve a trigger's real location before authorizing an authenticated runtime override.

use serde_json::json;

use crate::authorization::main::authorize_session::authorize_session;
use crate::authorization::main::deny_authorization::deny;
use crate::credentials::models::{Action, AuditContext, PermissionScope};
use crate::declarations::_helpers::operator_change::change_trigger_in;
use crate::declarations::models::{TriggerChange, TriggerChangeRequest};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

pub async fn change_person_trigger(
    store: &ControlStore,
    (context, credentials): (&AuditContext, (&str, &str)),
    request: &TriggerChangeRequest,
) -> Result<bool, ControlError> {
    let (transaction, rows) = query(begin(store, TransactionMode::Write).await?,
        "SELECT location_id,target FROM sqb_triggers WHERE trigger_id=$1 AND removed_in_snapshot_id IS NULL",
        vec![Parameter::Id(request.trigger_id)]).await?;
    let row = rows
        .first()
        .ok_or_else(|| ControlError::Invalid("current trigger not found".into()))?;
    let scope = PermissionScope {
        action: match request.change {
            TriggerChange::Enable | TriggerChange::Pause | TriggerChange::ResetDefault => {
                Action::TriggerToggle
            }
            TriggerChange::SetCursor { .. } | TriggerChange::ResetCursor => {
                Action::SensorResetCursor
            }
        },
        location_id: serde_json::from_value(row["location_id"].clone())
            .map_err(|error| ControlError::Invalid(error.to_string()))?,
        target: serde_json::from_value(row["target"].clone())
            .map_err(|error| ControlError::Invalid(error.to_string()))?,
        nodes: Default::default(),
    };
    let (transaction, context) =
        authorize_session(transaction, (context, credentials), &scope).await?;
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
