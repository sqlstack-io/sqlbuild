//! Creating a control store or applying its pending migrations, with the migration list as a parameter.

use uuid::Uuid;

use crate::store::constants::{INDEXES, SCHEMA};
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};
use crate::store_migrations::_helpers::migration::{
    migration_plan, migration_sql, recorded_schema, render, schema_digest, statements,
};
use crate::store_migrations::_helpers::recall_schema::recall_schema;
use crate::store_migrations::constants::BASELINE_VERSION;
use crate::store_migrations::models::{StoreMigration, StoreMigrationPlan};

/// `migrate_store` with the migrations `migrations` in place of this build's.
pub(crate) async fn migrate_with(
    store: &ControlStore,
    migrations: &[StoreMigration],
) -> Result<(), ControlError> {
    let transaction = begin(store, TransactionMode::Migrate).await?;
    let (mut transaction, recorded) = recorded_schema(store, transaction).await?;
    let plan = migration_plan(
        store,
        recorded
            .as_ref()
            .map(|(checksum, version)| (checksum.as_str(), *version)),
        migrations,
    )?;
    let digest = schema_digest(store);
    let now = transaction.now;
    match plan {
        StoreMigrationPlan::Current { .. } => {}
        StoreMigrationPlan::Create { version } => {
            (transaction, _) = query(transaction, &render(store,
                "CREATE TABLE sqb_schema_version (singleton BIGINT PRIMARY KEY CHECK (singleton = 1), \
                 checksum TEXT NOT NULL, applied_at {time} NOT NULL, version BIGINT)"), vec![]).await?;
            for statement in recall_schema(store) {
                (transaction, _) = query(transaction, &statement, vec![]).await?;
            }
            for statement in statements(SCHEMA) {
                (transaction, _) = query(transaction, &render(store, statement), vec![]).await?;
            }
            (transaction, _) = query(transaction,
                "INSERT INTO sqb_installation (singleton, installation_id, created_at) VALUES (1, $1, $2)",
                vec![Parameter::Id(Uuid::now_v7()), Parameter::Time(now)]).await?;
            (transaction, _) = query(
                transaction,
                "INSERT INTO sqb_schema_version (singleton, checksum, applied_at, version) \
                 VALUES (1, $1, $2, $3)",
                vec![
                    Parameter::Text(digest),
                    Parameter::Time(now),
                    Parameter::Integer(version),
                ],
            )
            .await?;
        }
        StoreMigrationPlan::Upgrade { from, to, .. } => {
            if recorded
                .as_ref()
                .is_some_and(|(_, version)| version.is_none())
            {
                (transaction, _) = query(
                    transaction,
                    "ALTER TABLE sqb_schema_version ADD COLUMN version BIGINT",
                    vec![],
                )
                .await?;
            }
            let pending = &migrations[usize::try_from(from - BASELINE_VERSION).unwrap_or(0)..];
            for migration in pending {
                let sql = migration_sql(store, migration);
                for statement in statements(&sql) {
                    (transaction, _) = query(transaction, statement, vec![]).await?;
                }
            }
            if !pending.is_empty() && matches!(store, ControlStore::Sqlite(_)) {
                let violations;
                (transaction, violations) =
                    query(transaction, "PRAGMA foreign_key_check", vec![]).await?;
                if !violations.is_empty() {
                    return Err(ControlError::Schema(format!(
                        "migrating the control store to version {to} would break {} foreign key \
                         references; the store is unchanged",
                        violations.len()
                    )));
                }
            }
            (transaction, _) = query(
                transaction,
                "UPDATE sqb_schema_version SET checksum = $1, applied_at = $2, version = $3",
                vec![
                    Parameter::Text(digest),
                    Parameter::Time(now),
                    Parameter::Integer(to),
                ],
            )
            .await?;
        }
    }
    for statement in statements(INDEXES) {
        (transaction, _) = query(transaction, statement, vec![]).await?;
    }
    commit(transaction).await
}
