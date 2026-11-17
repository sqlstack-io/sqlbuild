//! Proxy-provisioned people: ids derived from usernames, and the viewer each newcomer gets.

use uuid::Uuid;

use crate::authorization::_helpers::administrator_roles::assign_role_in;
use crate::credentials::main::identities::insert_principal::insert_principal;
use crate::credentials::models::{AuditContext, BuiltinRole, RoleAssignmentRequest, RoleKey};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// Prefixes each username before hashing, so proxy ids differ from every other derived id.
const PROXY_PRINCIPAL_PREFIX: &[u8] = b"sqlbuild:trusted-proxy-principal:";

/// The principal a proxy username provisions, derived so no table maps usernames to people.
pub(crate) fn proxy_principal_id(username: &str) -> Uuid {
    Uuid::new_v5(
        &Uuid::NAMESPACE_OID,
        &[PROXY_PRINCIPAL_PREFIX, username.as_bytes()].concat(),
    )
}

/// Whether `principal_id` exists, and if so whether it is enabled.
pub(crate) async fn principal_enabled(
    store: &ControlStore,
    principal_id: Uuid,
) -> Result<Option<bool>, ControlError> {
    let (transaction, people) = query(
        begin(store, TransactionMode::Read).await?,
        "SELECT disabled_at FROM sqb_principals WHERE principal_id=$1",
        vec![Parameter::Id(principal_id)],
    )
    .await?;
    commit(transaction).await?;
    Ok(people.first().map(|person| person["disabled_at"].is_null()))
}

/// Creates the person with the viewer role, or false when a principal's name matches in any case.
pub(crate) async fn provision_viewer(
    store: &ControlStore,
    context: &AuditContext,
    (principal_id, username): (Uuid, &str),
) -> Result<bool, ControlError> {
    let (transaction, taken) = query(
        begin(store, TransactionMode::Write).await?,
        "SELECT principal_id FROM sqb_principals WHERE lower(name)=$1",
        vec![Parameter::Text(username.into())],
    )
    .await?;
    if !taken.is_empty() {
        commit(transaction).await?;
        return Ok(false);
    }
    let (transaction, _) = insert_principal(transaction, context, (principal_id, username)).await?;
    let viewer = RoleAssignmentRequest {
        principal_id,
        role: RoleKey::Builtin(BuiltinRole::Viewer),
        location_id: None,
        target: None,
        revoke: false,
    };
    assign_role_in(transaction, context, &viewer).await?;
    Ok(true)
}
