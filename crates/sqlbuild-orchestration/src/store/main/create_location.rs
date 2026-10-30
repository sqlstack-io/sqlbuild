//! Create a server-owned location identity independent of its mutable label.

use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// Register a new location; names are labels, never credentials or stable identifiers.
pub async fn create_location(
    store: &ControlStore,
    name: &str,
    actor: &str,
) -> Result<Uuid, ControlError> {
    if name.trim().is_empty() || actor.trim().is_empty() {
        return Err(ControlError::Invalid(
            "location name and actor are required".into(),
        ));
    }
    let mut transaction = begin(store, TransactionMode::Write).await?;
    let id = Uuid::now_v7();
    let now = transaction.now;
    (transaction, _) = query(transaction,
        "INSERT INTO sqb_locations (location_id, name, created_by, created_at) VALUES ($1, $2, $3, $4)",
        vec![Parameter::Id(id), Parameter::Text(name.into()), Parameter::Text(actor.into()), Parameter::Time(now)]).await?;
    commit(transaction).await?;
    Ok(id)
}
