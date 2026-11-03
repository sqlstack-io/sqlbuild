//! Validate the complete initialized schema without migration, seeding or other writes.

use crate::store::_helpers::migration::{migration_digests, verify_migrations};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, ControlTransaction};

pub(crate) async fn verify_store(
    store: &ControlStore,
    transaction: ControlTransaction,
) -> Result<ControlTransaction, ControlError> {
    let digests = migration_digests(store);
    let (transaction, rows) = query(
        transaction,
        "SELECT version, checksum, min_server_version FROM sqb_schema_migrations ORDER BY version",
        vec![],
    )
    .await?;
    verify_migrations(&rows, &digests)?;
    if rows.len() != digests.len() {
        return Err(ControlError::Schema(
            "projection repair requires an initialized, fully upgraded control store".into(),
        ));
    }
    Ok(transaction)
}
