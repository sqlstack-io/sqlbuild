//! A person operation authorizes inside its own writer transaction, under one store lock.

use uuid::Uuid;

use crate::authorization::main::authorize_person::authorize_person;
use crate::credentials::models::{AuditContext, DefaultRole, PermissionScope};
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::models::{ControlStore, ControlTransaction, TransactionMode};

pub(crate) async fn begin_authorized(
    store: &ControlStore,
    credentials: (&AuditContext, Uuid),
    scope: &PermissionScope,
    default_role: DefaultRole,
) -> Result<(ControlTransaction, AuditContext), ControlError> {
    authorize_person(
        begin(store, TransactionMode::Write).await?,
        credentials,
        scope,
        default_role,
    )
    .await
}
