//! Shared location fixtures use the same audited identity operation as operator bootstrap.

use crate::credentials::models::AuditContext;
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;
use uuid::Uuid;

pub(crate) async fn create_location(
    store: &ControlStore,
    name: &str,
    actor: &str,
) -> Result<Uuid, ControlError> {
    crate::credentials::main::locations::create_location::create_location(
        store,
        name,
        &AuditContext {
            actor: actor.into(),
            granting_role: "admin".into(),
            request_id: Uuid::now_v7(),
            ip_address: None,
        },
    )
    .await
}
