//! Direct local store ownership may initialize the first administrator once, never reopen recovery.

use serde_json::json;
use uuid::Uuid;

use crate::authorization::_helpers::administrators::grant_installation_admin;
use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::main::identities::insert_principal::insert_principal;
use crate::credentials::models::{AuditContext, PrincipalKind};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

pub async fn bootstrap_user(
    store: &ControlStore,
    context: &AuditContext,
    name: &str,
) -> Result<Uuid, ControlError> {
    let (transaction, rows) = query(
        begin(store, TransactionMode::Write).await?,
        "SELECT principal_id FROM sqb_admin_initialization WHERE singleton=1",
        vec![],
    )
    .await?;
    if !rows.is_empty() {
        return Err(ControlError::Invalid(
            "administrator initialization already occurred; bootstrap is closed".into(),
        ));
    }
    let (transaction, principal_id) =
        insert_principal(transaction, context, (name, PrincipalKind::Person)).await?;
    let now = transaction.now;
    let (transaction, _) = query(transaction,
        "INSERT INTO sqb_admin_initialization (singleton,principal_id,initialized_at) VALUES (1,$1,$2)",
        vec![Parameter::Id(principal_id), Parameter::Time(now)]).await?;
    let (transaction, _) =
        grant_installation_admin(transaction, context, principal_id, "bootstrap").await?;
    commit(
        append_audit(
            transaction,
            context,
            (
                Uuid::new_v5(&principal_id, b"administrator-initialized"),
                "admin.initialized",
            ),
            (
                json!({"installation":true}),
                json!(null),
                json!({"principal_id":principal_id}),
            ),
        )
        .await?,
    )
    .await?;
    Ok(principal_id)
}
