//! The operator's CLI takes IDs and says what they name.

use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// The name of the location or principal `id` identifies, or `None` when neither does.
pub async fn recorded_name(store: &ControlStore, id: Uuid) -> Result<Option<String>, ControlError> {
    let (transaction, rows) = query(
        begin(store, TransactionMode::Read).await?,
        "SELECT name FROM sqb_locations WHERE location_id=$1 \
         UNION ALL SELECT name FROM sqb_principals WHERE principal_id=$1",
        vec![Parameter::Id(id)],
    )
    .await?;
    commit(transaction).await?;
    Ok(rows
        .first()
        .and_then(|row| row["name"].as_str())
        .map(str::to_owned))
}
