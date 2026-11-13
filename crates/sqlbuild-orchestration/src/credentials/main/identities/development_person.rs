//! `sqb dev` acts as one local administrator, recreated or restored on startup when missing.

use uuid::Uuid;

use crate::authorization::main::grant_admin::grant_admin;
use crate::credentials::main::identities::create_principal::create_principal;
use crate::credentials::models::AuditContext;
use crate::store::errors::ControlError;
use crate::store::main::read_records::read_records;
use crate::store::models::{ControlStore, Parameter};

pub async fn development_person(
    store: &ControlStore,
    context: &AuditContext,
) -> Result<Uuid, ControlError> {
    let rows: Vec<serde_json::Value> = read_records(
        store,
        "SELECT p.principal_id, p.disabled_at, a.assignment_id FROM sqb_principals p \
         LEFT JOIN sqb_role_assignments a ON a.principal_id=p.principal_id AND a.role_key='admin' \
         AND a.location_id IS NULL AND a.target IS NULL WHERE p.name=$1",
        vec![Parameter::Text("local-development".into())],
    )
    .await?;
    let id = match rows.first() {
        Some(row) => serde_json::from_value(row["principal_id"].clone())
            .map_err(|_| ControlError::Schema("invalid development person identity".into()))?,
        None => create_principal(store, context, "local-development").await?,
    };
    let administrator = rows
        .first()
        .is_some_and(|row| row["disabled_at"].is_null() && !row["assignment_id"].is_null());
    if !administrator {
        grant_admin(store, context, id).await?;
    }
    Ok(id)
}
