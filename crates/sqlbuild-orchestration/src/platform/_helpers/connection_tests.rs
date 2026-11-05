//! Connection-test submission and reads share live action authority under the store lock.

use uuid::Uuid;

use crate::authorization::main::authorize_person::authorize_person;
use crate::credentials::models::{Action, AuditContext, DefaultRole, PermissionScope};
use crate::store::errors::ControlError;
use crate::store::models::ControlTransaction;

/// No credentials means a trusted direct-store owner, never an HTTP-supplied role.
pub(crate) async fn authorize_connection_test(
    transaction: ControlTransaction,
    (context, credentials): (&AuditContext, Option<Uuid>),
    (location_id, target): (Uuid, &str),
) -> Result<(ControlTransaction, AuditContext), ControlError> {
    let Some(credentials) = credentials else {
        return Ok((transaction, context.clone()));
    };
    authorize_person(
        transaction,
        (context, credentials),
        &PermissionScope::action(Action::ConnectionTest, Some((location_id, target))),
        DefaultRole::None,
    )
    .await
}
