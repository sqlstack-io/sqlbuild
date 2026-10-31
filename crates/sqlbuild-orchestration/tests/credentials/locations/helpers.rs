//! Shared location fixtures use the same audited identity operation as operator bootstrap.

use sqlbuild_orchestration::credentials::models::AuditContext;
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::store::models::ControlStore;
use uuid::Uuid;

pub(crate) async fn create_location(
    store: &ControlStore,
    name: &str,
    actor: &str,
) -> Result<Uuid, ControlError> {
    sqlbuild_orchestration::credentials::main::create_location::create_location(
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
