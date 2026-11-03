//! Authentication denials receive durable decision identities without credential material.

use serde_json::{Value, json};
use uuid::Uuid;

use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::models::AuditContext;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

pub async fn record_authentication_denial(
    store: &ControlStore,
    context: &AuditContext,
    (decision_id, reason): (Uuid, &str),
    scope: Value,
) -> Result<Uuid, ControlError> {
    commit(
        append_audit(
            begin(store, TransactionMode::Write).await?,
            context,
            (decision_id, "authentication.denied"),
            (
                scope,
                json!(null),
                json!({"allowed": false, "reason": reason}),
            ),
        )
        .await?,
    )
    .await?;
    Ok(decision_id)
}
