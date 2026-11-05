//! Validate the complete initialized schema without migration, seeding or other writes.

use crate::store::_helpers::migration::{schema_digest, verify_schema};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, ControlTransaction};

pub(crate) async fn verify_store(
    store: &ControlStore,
    transaction: ControlTransaction,
) -> Result<ControlTransaction, ControlError> {
    let (transaction, rows) = query(
        transaction,
        "SELECT checksum, min_server_version FROM sqb_schema_version",
        vec![],
    )
    .await?;
    verify_schema(&rows, &schema_digest(store))?;
    if rows.is_empty() {
        return Err(ControlError::Schema(
            "projection repair requires an initialized control store".into(),
        ));
    }
    Ok(transaction)
}
