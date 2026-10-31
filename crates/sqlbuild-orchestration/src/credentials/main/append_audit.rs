//! Security changes and their audit records commit together, without bearer material.

use serde_json::Value;
use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn append_audit(
    transaction: ControlTransaction,
    context: &AuditContext,
    (event_id, action): (Uuid, &str),
    (scope, before, after): (Value, Value, Value),
) -> Result<ControlTransaction, ControlError> {
    if context.actor.trim().is_empty() || context.granting_role.trim().is_empty() {
        return Err(ControlError::Invalid(
            "audit actor and granting role are required".into(),
        ));
    }
    let now = transaction.now;
    let (transaction, _) = query(transaction,
        "INSERT INTO sqb_security_audit (event_id, occurred_at, actor, action, granting_role, request_id, ip_address, scope_json, before_json, after_json) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
        vec![Parameter::Id(event_id), Parameter::Time(now), Parameter::Text(context.actor.clone()),
            Parameter::Text(action.into()), Parameter::Text(context.granting_role.clone()), Parameter::Id(context.request_id),
            Parameter::OptionalText(context.ip_address.map(|value| value.to_string())),
            Parameter::Json(scope), Parameter::Json(before), Parameter::Json(after)]).await?;
    Ok(transaction)
}
