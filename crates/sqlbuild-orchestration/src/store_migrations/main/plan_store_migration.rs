//! What opening a control store would do to its schema, without changing it.

use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::models::{ControlStore, TransactionMode};
use crate::store_migrations::_helpers::migration::{MIGRATIONS, migration_plan, recorded_schema};
use crate::store_migrations::models::StoreMigrationPlan;

/// The schema change `migrate_store` would make to `store`, or why it would refuse the store.
pub async fn plan_store_migration(
    store: &ControlStore,
) -> Result<StoreMigrationPlan, ControlError> {
    let transaction = begin(store, TransactionMode::Read).await?;
    let (transaction, recorded) = recorded_schema(store, transaction).await?;
    commit(transaction).await?;
    migration_plan(
        store,
        recorded
            .as_ref()
            .map(|(checksum, version)| (checksum.as_str(), *version)),
        MIGRATIONS,
    )
}
