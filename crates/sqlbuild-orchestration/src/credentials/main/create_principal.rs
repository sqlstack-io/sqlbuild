//! Server-owned people and service accounts; creating one never grants a role implicitly.

use serde_json::json;
use uuid::Uuid;

use crate::credentials::main::append_audit::append_audit;
use crate::credentials::models::{AuditContext, PrincipalKind};
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};

pub async fn create_principal(
    store: &ControlStore,
    context: &AuditContext,
    (name, kind): (&str, PrincipalKind),
) -> Result<Uuid, ControlError> {
    if name.trim().is_empty() {
        return Err(ControlError::Invalid("principal name is required".into()));
    }
    let transaction = begin(store, TransactionMode::Write).await?;
    let id = Uuid::now_v7();
    let now = transaction.now;
    let kind = match kind {
        PrincipalKind::Person => "person",
        PrincipalKind::ServiceAccount => "service_account",
    };
    let (transaction, _) = query(
        transaction,
        "INSERT INTO sqb_principals (principal_id, name, kind, created_at) VALUES ($1,$2,$3,$4)",
        vec![
            Parameter::Id(id),
            Parameter::Text(name.into()),
            Parameter::Text(kind.into()),
            Parameter::Time(now),
        ],
    )
    .await?;
    commit(
        append_audit(
            transaction,
            context,
            (Uuid::new_v5(&id, b"created"), "principal.created"),
            (
                json!({"principal_id": id}),
                json!(null),
                json!({"name": name, "kind": kind}),
            ),
        )
        .await?,
    )
    .await?;
    Ok(id)
}
