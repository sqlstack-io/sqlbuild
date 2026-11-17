//! Create a role-free identity; bootstrap authority is a separate, one-time operation.

use serde_json::json;
use uuid::Uuid;

use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::models::AuditContext;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

/// Creates principal `name` with `id`: a fresh v7 id, or one derived from an external username.
pub(crate) async fn insert_principal(
    transaction: ControlTransaction,
    context: &AuditContext,
    (id, name): (Uuid, &str),
) -> Result<(ControlTransaction, Uuid), ControlError> {
    if name.trim().is_empty() {
        return Err(ControlError::Invalid("principal name is required".into()));
    }
    let now = transaction.now;
    let (transaction, _) = query(
        transaction,
        "INSERT INTO sqb_principals (principal_id, name, created_at) VALUES ($1,$2,$3)",
        vec![
            Parameter::Id(id),
            Parameter::Text(name.into()),
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
            json!({"name": name}),
        ),
    )
    .await?;
    Ok((transaction, id))
}
