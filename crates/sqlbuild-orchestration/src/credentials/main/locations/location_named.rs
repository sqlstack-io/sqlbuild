//! The installation's launcher configuration names a location; the store holds its identity.

use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// The active location with this name, or `None` when none exists or it is archived.
pub async fn location_named(
    store: &ControlStore,
    name: &str,
) -> Result<Option<Uuid>, ControlError> {
    let (transaction, rows) = query(
        begin(store, TransactionMode::Read).await?,
        "SELECT location_id FROM sqb_locations WHERE name=$1 AND archived_at IS NULL",
        vec![Parameter::Text(name.into())],
    )
    .await?;
    commit(transaction).await?;
    rows.first()
        .map(|row| {
            row["location_id"]
                .as_str()
                .unwrap_or_default()
                .parse()
                .map_err(|_| ControlError::Schema("invalid location identity".into()))
        })
        .transpose()
}
