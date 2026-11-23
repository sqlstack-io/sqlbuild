//! A known key that is refused is audited under its service account, so its owner sees the failed calls.

use serde_json::json;
use uuid::Uuid;

use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::models::AuditContext;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

/// Records that `key_id` of `principal_id` was refused for `reason`, as that service account.
pub(crate) async fn record_refused_key(
    store: &ControlStore,
    context: &AuditContext,
    (principal_id, key_id, reason): (Uuid, Uuid, &str),
) -> Result<(), ControlError> {
    let context = AuditContext {
        actor: format!("principal:{principal_id}"),
        granting_role: "service-key".into(),
        ..context.clone()
    };
    let transaction = append_audit(
        begin(store, TransactionMode::Write).await?,
        &context,
        (Uuid::now_v7(), "service_account.key_refused"),
        (
            json!({"principal_id": principal_id}),
            json!(null),
            json!({"key_id": key_id, "allowed": false, "reason": reason}),
        ),
    )
    .await?;
    commit(transaction).await
}
