//! Forward-only migration under the same lock used by all other control-plane writes.

use semver::Version;
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;
use uuid::Uuid;

use crate::store::_helpers::migration::{SCHEMA, render};
use crate::store::_helpers::recall_schema::recall_schema;
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// Create the schema once, or reject drift and schemas newer than this server supports.
pub async fn migrate_store(store: &ControlStore) -> Result<(), ControlError> {
    let mut transaction = begin(store, TransactionMode::Write).await?;
    (transaction, _) = query(transaction, &render(store,
        "CREATE TABLE IF NOT EXISTS sqb_schema_migrations (version BIGINT PRIMARY KEY, \
         name TEXT NOT NULL, checksum TEXT NOT NULL, min_server_version TEXT NOT NULL, applied_at {time} NOT NULL)"), vec![]).await?;
    let (mut transaction, rows) = query(
        transaction,
        "SELECT version, checksum, min_server_version FROM sqb_schema_migrations ORDER BY version",
        vec![],
    )
    .await?;
    let current = Version::parse(env!("CARGO_PKG_VERSION"))
        .map_err(|error| ControlError::Schema(error.to_string()))?;
    let recall_statements = recall_schema(store);
    let digest = sha256_hex(format!("{SCHEMA}\n{}", recall_statements.join(";\n")).as_bytes());
    for row in &rows {
        let minimum = row
            .get("min_server_version")
            .and_then(|value| value.as_str())
            .ok_or_else(|| {
                ControlError::Schema("migration lacks a minimum server version".into())
            })?;
        let minimum =
            Version::parse(minimum).map_err(|error| ControlError::Schema(error.to_string()))?;
        if row.get("version").and_then(|value| value.as_i64()) != Some(1)
            || minimum > current
            || row.get("checksum").and_then(|value| value.as_str()) != Some(digest.as_str())
        {
            return Err(ControlError::Schema(
                "unknown migration, checksum mismatch, or minimum server version not met".into(),
            ));
        }
    }
    if rows.is_empty() {
        for statement in SCHEMA
            .split(';')
            .map(str::trim)
            .filter(|statement| !statement.is_empty())
        {
            (transaction, _) = query(transaction, &render(store, statement), vec![]).await?;
        }
        for statement in recall_statements {
            (transaction, _) = query(transaction, &statement, vec![]).await?;
        }
        let now = transaction.now;
        (transaction, _) = query(transaction,
            "INSERT INTO sqb_installation (singleton, installation_id, created_at) VALUES (1, $1, $2)",
            vec![Parameter::Id(Uuid::now_v7()), Parameter::Time(now)]).await?;
        (transaction, _) = query(transaction,
            "INSERT INTO sqb_schema_migrations (version, name, checksum, min_server_version, applied_at) \
             VALUES (1, 'control_store', $1, $2, $3)",
            vec![Parameter::Text(digest), Parameter::Text(current.to_string()), Parameter::Time(now)]).await?;
    }
    commit(transaction).await
}
