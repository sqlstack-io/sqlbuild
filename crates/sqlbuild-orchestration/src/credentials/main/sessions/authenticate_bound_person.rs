//! Explicit OIDC bindings issue browser sessions without provisioning accounts or roles.

use serde_json::json;
use uuid::Uuid;

use crate::credentials::_helpers::browser_sessions::issue_live_session;
use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::models::{AuditContext, IssuedSession};
use crate::store::errors::ControlError;
use crate::store::main::write::write;
use crate::store::models::ControlStore;

pub(crate) async fn authenticate_bound_person(
    store: &ControlStore,
    context: &AuditContext,
    principal_id: Uuid,
) -> Result<Option<IssuedSession>, ControlError> {
    write(store, |transaction| async move {
        let (transaction, issued) =
            issue_live_session(transaction, context, (principal_id, None), "oidc").await?;
        let Some((context, session)) = issued else {
            return Ok((transaction, None));
        };
        let identity = json!({"principal_id": principal_id});
        let transaction = append_audit(
            transaction,
            &context,
            (context.request_id, "oidc.authenticated"),
            (identity.clone(), json!(null), identity),
        )
        .await?;
        Ok((transaction, Some(session)))
    })
    .await
}
