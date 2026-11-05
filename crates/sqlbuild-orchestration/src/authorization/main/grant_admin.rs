//! Local store ownership restores an existing person as administrator, never reopens bootstrap.

use serde_json::json;
use uuid::Uuid;

use crate::authorization::_helpers::administrators::grant_installation_admin;
use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::main::sessions::revoke_session::revoke_session;
use crate::credentials::models::AuditContext;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{Change, ControlStore, Parameter, TransactionMode};

pub async fn grant_admin(
    store: &ControlStore,
    context: &AuditContext,
    principal_id: Uuid,
) -> Result<(), ControlError> {
    let (transaction, initialized) = query(
        begin(store, TransactionMode::Write).await?,
        "SELECT principal_id FROM sqb_admin_initialization WHERE singleton=1",
        vec![],
    )
    .await?;
    if initialized.is_empty() {
        return Err(ControlError::Invalid(
            "administrator initialization has not occurred; use create-user".into(),
        ));
    }
    let (transaction, people) = query(
        transaction,
        "SELECT disabled_at FROM sqb_principals WHERE principal_id=$1",
        vec![Parameter::Id(principal_id)],
    )
    .await?;
    let Some(person) = people.first() else {
        return Err(ControlError::Invalid(
            "grant-admin requires an existing person".into(),
        ));
    };
    let enabled = person["disabled_at"].is_null();
    let (transaction, _) = query(
        transaction,
        "UPDATE sqb_principals SET disabled_at=NULL WHERE principal_id=$1",
        vec![Parameter::Id(principal_id)],
    )
    .await?;
    let (transaction, _) =
        grant_installation_admin(transaction, context, principal_id, "local").await?;
    let (mut transaction, sessions_revoked) =
        revoke_session(transaction, context, (principal_id, None)).await?;
    transaction = append_audit(
        transaction,
        context,
        (Uuid::now_v7(), "admin.granted"),
        (
            json!({"principal_id":principal_id}),
            json!({"enabled":enabled}),
            json!({"enabled":true,"role":"admin","sessions_revoked":sessions_revoked}),
        ),
    )
    .await?;
    transaction.changes.insert(Change::Credentials);
    commit(transaction).await
}
