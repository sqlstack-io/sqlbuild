//! Another administrator decides a widening; a sole administrator may confirm a self-approval.

use serde_json::json;

use crate::authorization::_helpers::role_approvals::{
    administrator_count, pending_request, record_decision,
};
use crate::authorization::main::authorize_session::authorize_session;
use crate::authorization::main::deny_authorization::deny;
use crate::credentials::models::{
    Action, AuditContext, DefaultRole, PermissionScope, RoleApproval,
};
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::models::{ControlStore, TransactionMode};

pub async fn decide_role_approval(
    store: &ControlStore,
    (context, credentials): (&AuditContext, (&str, &str)),
    approval: &RoleApproval,
) -> Result<bool, ControlError> {
    let scope = PermissionScope {
        action: Action::RoleAssign,
        location_id: None,
        target: None,
        nodes: Default::default(),
    };
    let (transaction, context) = authorize_session(
        begin(store, TransactionMode::Write).await?,
        (context, credentials),
        &scope,
        DefaultRole::None,
    )
    .await?;
    if approval.reason.trim().is_empty() {
        return Err(ControlError::Invalid("role approval needs a reason".into()));
    }
    let (transaction, request) = pending_request(transaction, approval.request_id).await?;
    let self_approval = request["requested_by"].as_str() == Some(context.actor.as_str());
    let (transaction, administrators) = administrator_count(transaction).await?;
    if self_approval && (administrators != 1 || !approval.confirm_self_approval) {
        return deny(
            transaction,
            &context,
            json!(approval),
            "self-approval needs explicit confirmation and exactly one administrator; another administrator must decide",
        )
        .await;
    }
    record_decision(transaction, &context, approval, (request, self_approval)).await
}
