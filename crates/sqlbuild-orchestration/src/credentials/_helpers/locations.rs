//! Create a server-owned location identity independent of its mutable label.

use serde_json::json;
use uuid::Uuid;

use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::models::AuditContext;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn create_location_in(
    mut transaction: ControlTransaction,
    name: &str,
    context: &AuditContext,
) -> Result<(ControlTransaction, Uuid), ControlError> {
    if name.trim().is_empty() {
        return Err(ControlError::Invalid("location name is required".into()));
    }
    let id = Uuid::now_v7();
    let now = transaction.now;
    (transaction, _) = query(transaction,
        "INSERT INTO sqb_locations (location_id, name, created_by, created_at) VALUES ($1, $2, $3, $4)",
        vec![Parameter::Id(id), Parameter::Text(name.into()), Parameter::Text(context.actor.clone()), Parameter::Time(now)]).await?;
    let transaction = append_audit(
        transaction,
        context,
        (Uuid::new_v5(&id, b"created"), "location.created"),
        (
            json!({"location_id": id}),
            json!(null),
            json!({"name": name}),
        ),
    )
    .await?;
    Ok((transaction, id))
}
