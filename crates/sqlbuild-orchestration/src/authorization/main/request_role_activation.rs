//! A role assigner records which widened custom role definition should take effect.

use serde_json::json;
use uuid::Uuid;

use crate::authorization::_helpers::custom_roles::{current_role, effective_role};
use crate::authorization::main::authorize_session::authorize_session;
use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::models::{
    Action, AuditContext, DefaultRole, PermissionScope, RoleActivation, RoleActivationRequest,
};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

pub async fn request_role_activation(
    store: &ControlStore,
    (context, credentials): (&AuditContext, (&str, &str)),
    request: &RoleActivationRequest,
) -> Result<RoleActivation, ControlError> {
    let (transaction, context) = authorize_session(
        begin(store, TransactionMode::Write).await?,
        (context, credentials),
        &PermissionScope {
            action: Action::RoleAssign,
            location_id: Some(request.location_id),
            target: Some(request.target.clone()),
            nodes: Default::default(),
        },
        DefaultRole::None,
    )
    .await?;
    if request.reason.trim().is_empty() {
        return Err(ControlError::Invalid(
            "role activation needs a reason".into(),
        ));
    }
    let scope = (request.location_id, request.target.as_str());
    let (transaction, role) = current_role(transaction, scope, &request.role_key).await?;
    let Some(role) = role else {
        return Err(ControlError::Invalid(format!(
            "role {} is not in the current published definitions",
            request.role_key
        )));
    };
    let (transaction, effective) = effective_role(transaction, scope, &request.role_key).await?;
    if effective
        .as_ref()
        .is_some_and(|(_, grants)| role.grants.is_subset(grants))
    {
        return Err(ControlError::Invalid(format!(
            "role {} does not widen its effective definition; narrowing applies on publish",
            request.role_key
        )));
    }
    let (request_id, now) = (Uuid::now_v7(), transaction.now);
    let (transaction, _) = query(transaction,
        "INSERT INTO sqb_role_activation_requests (request_id,location_id,target,role_key,definition_digest,requested_by,requested_at,reason) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
        vec![Parameter::Id(request_id), Parameter::Id(request.location_id), Parameter::Text(request.target.clone()),
            Parameter::Text(request.role_key.clone()), Parameter::Text(role.digest.clone()),
            Parameter::Text(context.actor.clone()), Parameter::Time(now), Parameter::Text(request.reason.clone())]).await?;
    let transaction = append_audit(
        transaction,
        &context,
        (request_id, "role.activation_requested"),
        (
            json!(request),
            json!(
                effective
                    .map(|(digest, grants)| json!({"definition_digest": digest, "grants": grants}))
            ),
            json!({"definition_digest": role.digest, "grants": role.grants}),
        ),
    )
    .await?;
    commit(transaction).await?;
    Ok(RoleActivation {
        request_id,
        definition_digest: role.digest,
    })
}
