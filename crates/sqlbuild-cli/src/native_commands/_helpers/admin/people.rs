//! Commands that change a person by ID and say whom they changed.

use sqlbuild_orchestration::authorization::main::roles::grant_admin::grant_admin;
use sqlbuild_orchestration::credentials::main::sessions::set_password::set_password;
use sqlbuild_orchestration::credentials::models::{AuditContext, PasswordAuthority};
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::store::models::ControlStore;
use uuid::Uuid;

use crate::native_commands::_helpers::command::admin_actions::name_or_id;
use crate::native_commands::models::AdminDone;

pub(crate) async fn provide_password(
    store: &ControlStore,
    context: &AuditContext,
    (id, password): (Uuid, String),
) -> Result<AdminDone, ControlError> {
    let name: String = name_or_id(store, id).await?;
    let authority = PasswordAuthority::Operator(id);
    set_password(store, context, authority, &password).await?;
    Ok(AdminDone::PasswordSet { id, name })
}

pub(crate) async fn restore_admin(
    store: &ControlStore,
    context: &AuditContext,
    id: Uuid,
) -> Result<AdminDone, ControlError> {
    let name: String = name_or_id(store, id).await?;
    grant_admin(store, context, id).await?;
    Ok(AdminDone::AdminGranted { id, name })
}
