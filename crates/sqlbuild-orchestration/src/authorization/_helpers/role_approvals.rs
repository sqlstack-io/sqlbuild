//! Approval rows for custom role activation requests, and the effective role they activate.

use serde_json::json;
use uuid::Uuid;

use crate::authorization::_helpers::custom_roles::{
    current_role, effective_role, retained_grants, set_effective_role,
};
use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::models::{AuditContext, RoleApproval, RoleDecision};
use crate::store::errors::ControlError;
use crate::store::main::{commit::commit, query::query};
use crate::store::models::{ControlTransaction, Parameter};
use crate::store::types::Row;

pub(crate) async fn administrator_count(
    transaction: ControlTransaction,
) -> Result<(ControlTransaction, i64), ControlError> {
    let (transaction, rows) = query(transaction,
        "SELECT COUNT(*) AS administrators FROM sqb_role_assignments a JOIN sqb_principals p USING(principal_id) \
         WHERE a.role_key='admin' AND a.location_id IS NULL AND a.target IS NULL AND a.revoked_at IS NULL \
         AND p.kind='person' AND p.disabled_at IS NULL", vec![]).await?;
    let count = rows
        .first()
        .and_then(|row| row["administrators"].as_i64())
        .unwrap_or_default();
    Ok((transaction, count))
}

pub(crate) async fn pending_request(
    transaction: ControlTransaction,
    request_id: Uuid,
) -> Result<(ControlTransaction, Row), ControlError> {
    let (transaction, rows) = query(transaction,
        "SELECT r.location_id,r.target,r.role_key,r.definition_digest,r.requested_by,a.approval_id FROM sqb_role_activation_requests r \
         LEFT JOIN sqb_role_approvals a ON a.request_id=r.request_id WHERE r.request_id=$1",
        vec![Parameter::Id(request_id)]).await?;
    match rows.into_iter().next() {
        Some(row) if row["approval_id"].is_null() => Ok((transaction, row)),
        Some(_) => Err(ControlError::Invalid(
            "role activation request is already decided".into(),
        )),
        None => Err(ControlError::Invalid(
            "unknown role activation request".into(),
        )),
    }
}

pub(crate) async fn record_decision(
    transaction: ControlTransaction,
    context: &AuditContext,
    approval: &RoleApproval,
    (request, self_approval): (Row, bool),
) -> Result<bool, ControlError> {
    let location_id: Uuid = serde_json::from_value(request["location_id"].clone())
        .map_err(|error| ControlError::Schema(error.to_string()))?;
    let target = request["target"].as_str().unwrap_or_default();
    let role_key = request["role_key"].as_str().unwrap_or_default();
    let digest = request["definition_digest"].as_str().unwrap_or_default();
    let (transaction, before) =
        effective_role(transaction, (location_id, target), role_key).await?;
    let (mut transaction, role) =
        current_role(transaction, (location_id, target), role_key).await?;
    let approved = approval.decision == RoleDecision::Approved;
    if approved && role.as_ref().is_none_or(|role| role.digest != digest) {
        return Err(ControlError::Invalid(
            "a newer publication superseded this role definition; request activation again".into(),
        ));
    }
    let (approval_id, now) = (Uuid::now_v7(), transaction.now);
    let decision = serde_json::to_value(approval.decision)
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    (transaction, _) = query(transaction,
        "INSERT INTO sqb_role_approvals (approval_id,request_id,location_id,target,role_key,definition_digest,decision,decided_by,decided_at,reason,signature,self_approval) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,NULL,$11)",
        vec![Parameter::Id(approval_id), Parameter::Id(approval.request_id), Parameter::Id(location_id),
            Parameter::Text(target.into()), Parameter::Text(role_key.into()), Parameter::Text(digest.into()),
            Parameter::Text(decision.as_str().unwrap_or_default().into()), Parameter::Text(context.actor.clone()),
            Parameter::Time(now), Parameter::Text(approval.reason.clone()), Parameter::Boolean(self_approval)]).await?;
    if let Some(role) = role.filter(|_| approved) {
        transaction = set_effective_role(
            transaction,
            (location_id, target, role_key),
            (digest, &retained_grants(&role.grants, &role.grants)),
        )
        .await?;
    }
    let after = json!({"decision": decision, "definition_digest": digest, "signed": false,
        "self_approval": self_approval, "requested_by": request["requested_by"]});
    transaction = append_audit(
        transaction,
        context,
        (
            approval_id,
            if approved {
                "role.approved"
            } else {
                "role.rejected"
            },
        ),
        (
            json!(approval),
            json!(
                before
                    .map(|(digest, grants)| json!({"definition_digest": digest, "grants": grants}))
            ),
            after,
        ),
    )
    .await?;
    commit(transaction).await?;
    Ok(true)
}
