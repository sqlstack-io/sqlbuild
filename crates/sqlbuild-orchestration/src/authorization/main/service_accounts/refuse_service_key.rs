//! A service account never manages service accounts or roles, so a leaked key can't plant lasting access.

use serde_json::json;

use crate::authorization::main::decisions::deny_authorization::deny;
use crate::credentials::models::AuditContext;
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::models::{ControlStore, TransactionMode};

/// Refuses `action` for the service account in `context`, audited under its name.
pub(crate) async fn refuse_service_key(
    store: &ControlStore,
    context: &AuditContext,
    action: &str,
) -> Result<(), ControlError> {
    deny(
        begin(store, TransactionMode::Write).await?,
        context,
        json!({"action": action, "caller": "service_account"}),
        "service accounts can't manage service accounts or roles, whatever their role; a person \
         must make this change",
    )
    .await
}
