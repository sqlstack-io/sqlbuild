//! Federation proves identity only; explicit bindings never provision accounts or grant roles.

use serde_json::json;
use uuid::Uuid;

use crate::credentials::main::append_audit::append_audit;
use crate::credentials::models::{AuditContext, ExternalIdentityMechanism};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

pub(crate) async fn authenticate_bound_person(
    store: &ControlStore,
    context: &AuditContext,
    principal_id: Uuid,
    mechanism: ExternalIdentityMechanism,
) -> Result<Option<Uuid>, ControlError> {
    let transaction = begin(store, TransactionMode::Write).await?;
    let (transaction, rows) = query(transaction,
        "SELECT principal_id FROM sqb_principals WHERE principal_id=$1 AND kind='person' AND disabled_at IS NULL",
        vec![Parameter::Id(principal_id)]).await?;
    if rows.is_empty() {
        commit(transaction).await?;
        return Ok(None);
    }
    let (action, granting_role) = match mechanism {
        ExternalIdentityMechanism::TrustedProxy => ("proxy.authenticated", "trusted-proxy"),
        ExternalIdentityMechanism::Oidc => ("oidc.authenticated", "oidc"),
    };
    let context = AuditContext {
        actor: format!("principal:{principal_id}"),
        granting_role: granting_role.into(),
        ..context.clone()
    };
    let identity = json!({"principal_id": principal_id});
    commit(
        append_audit(
            transaction,
            &context,
            (context.request_id, action),
            (identity.clone(), json!(null), identity),
        )
        .await?,
    )
    .await?;
    Ok(Some(principal_id))
}
