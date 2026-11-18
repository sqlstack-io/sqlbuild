//! Schema creation and numbered migrations under the same lock used by all other control-plane writes.

use crate::store::errors::ControlError;
use crate::store::models::ControlStore;
use crate::store_migrations::main::apply_store_migrations::apply_store_migrations;

/// Create the schema or apply its pending migrations in one transaction, then add any missing additive indexes.
pub async fn migrate_store(store: &ControlStore) -> Result<(), ControlError> {
    apply_store_migrations(store).await
}
