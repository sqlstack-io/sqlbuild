//! Schema creation under the same lock used by all other control-plane writes.

use uuid::Uuid;

use crate::store::_helpers::migration::{
    INDEXES, SCHEMA, render, schema_digest, statements, verify_schema,
};
use crate::store::_helpers::recall_schema::recall_schema;
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// Create or verify the schema, then add any missing additive indexes.
pub async fn migrate_store(store: &ControlStore) -> Result<(), ControlError> {
    let mut transaction = begin(store, TransactionMode::Write).await?;
    (transaction, _) = query(transaction, &render(store,
        "CREATE TABLE IF NOT EXISTS sqb_schema_version (singleton BIGINT PRIMARY KEY CHECK (singleton = 1), \
         checksum TEXT NOT NULL, applied_at {time} NOT NULL)"), vec![]).await?;
    let (mut transaction, rows) = query(
        transaction,
        "SELECT checksum FROM sqb_schema_version",
        vec![],
    )
    .await?;
    let digest = schema_digest(store);
    verify_schema(store, &rows, &digest)?;
    if rows.is_empty() {
        for statement in recall_schema(store) {
            (transaction, _) = query(transaction, &statement, vec![]).await?;
        }
        for statement in statements(SCHEMA) {
            (transaction, _) = query(transaction, &render(store, statement), vec![]).await?;
        }
        let now = transaction.now;
        (transaction, _) = query(transaction,
            "INSERT INTO sqb_installation (singleton, installation_id, created_at) VALUES (1, $1, $2)",
            vec![Parameter::Id(Uuid::now_v7()), Parameter::Time(now)]).await?;
        (transaction, _) = query(
            transaction,
            "INSERT INTO sqb_schema_version (singleton, checksum, applied_at) VALUES (1, $1, $2)",
            vec![Parameter::Text(digest), Parameter::Time(now)],
        )
        .await?;
    }
    for statement in statements(INDEXES) {
        (transaction, _) = query(transaction, statement, vec![]).await?;
    }
    commit(transaction).await
}
