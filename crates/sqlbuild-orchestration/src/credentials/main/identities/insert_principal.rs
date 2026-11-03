//! Create a role-free identity; bootstrap authority is a separate, one-time operation.

use serde_json::json;
use uuid::Uuid;

use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::models::{AuditContext, PrincipalKind};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn insert_principal(
    transaction: ControlTransaction,
    context: &AuditContext,
    (name, kind): (&str, PrincipalKind),
) -> Result<(ControlTransaction, Uuid), ControlError> {
    if name.trim().is_empty() {
        return Err(ControlError::Invalid("principal name is required".into()));
    }
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
    let transaction = append_audit(
        transaction,
        context,
        (Uuid::new_v5(&id, b"created"), "principal.created"),
        (
            json!({"principal_id": id}),
            json!(null),
            json!({"name": name, "kind": kind}),
        ),
    )
    .await?;
    Ok((transaction, id))
}
