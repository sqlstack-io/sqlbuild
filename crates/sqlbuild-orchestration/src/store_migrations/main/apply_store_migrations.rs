//! Bring a control store to this build's schema version.

use crate::store::errors::ControlError;
use crate::store::models::ControlStore;
use crate::store_migrations::_helpers::migration::MIGRATIONS;
use crate::store_migrations::_helpers::migration_run::migrate_with;

/// Create the schema or apply this build's pending migrations in one transaction, then add any missing additive indexes.
pub(crate) async fn apply_store_migrations(store: &ControlStore) -> Result<(), ControlError> {
    migrate_with(store, MIGRATIONS).await
}
