//! Snapshot expiry is by reference: anything a retained row points at stays, and content follows.

use uuid::Uuid;

use crate::snapshots::constants::{
    CONTENT, CONTENT_HASH_CHUNK, EXPIRED_SNAPSHOTS, SNAPSHOT_RETENTION_BATCH,
};
use crate::snapshots::models::SnapshotRetention;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, ControlTransaction, Parameter, TransactionMode};

/// Batches commit independently; a repeated or interrupted pass finds only what is still expired.
pub(crate) async fn prune_snapshots(
    store: &ControlStore,
    retention: SnapshotRetention,
) -> Result<usize, ControlError> {
    let mut deleted = 0;
    loop {
        let transaction = begin(store, TransactionMode::Write).await?;
        let (transaction, rows) = query(
            transaction,
            EXPIRED_SNAPSHOTS,
            vec![
                Parameter::Integer(retention.kept()),
                Parameter::Integer(SNAPSHOT_RETENTION_BATCH as i64),
            ],
        )
        .await?;
        let mut expired: Vec<Uuid> = Vec::with_capacity(rows.len());
        for row in &rows {
            expired.push(
                serde_json::from_value(row["snapshot_id"].clone())
                    .map_err(|error| ControlError::Schema(error.to_string()))?,
            );
        }
        let transaction = delete_snapshots(transaction, &expired).await?;
        commit(transaction).await?;
        deleted += expired.len();
        if expired.len() < SNAPSHOT_RETENTION_BATCH {
            return Ok(deleted);
        }
    }
}

async fn delete_snapshots(
    mut transaction: ControlTransaction,
    expired: &[Uuid],
) -> Result<ControlTransaction, ControlError> {
    if expired.is_empty() {
        return Ok(transaction);
    }
    let expired_list = Parameter::placeholders(1..=expired.len());
    let ids: Vec<Parameter> = expired.iter().copied().map(Parameter::Id).collect();
    let mut hashes: Vec<Vec<String>> = Vec::with_capacity(CONTENT.len());
    for ((table, column), _, _) in CONTENT {
        let rows;
        (transaction, rows) = query(
            transaction,
            &format!("SELECT DISTINCT {column} FROM {table} WHERE snapshot_id IN ({expired_list})"),
            ids.clone(),
        )
        .await?;
        hashes.push(
            rows.iter()
                .filter_map(|row| row[column].as_str().map(str::to_owned))
                .collect(),
        );
    }
    for table in [
        "sqb_snapshot_sources",
        "sqb_snapshot_nodes",
        "sqb_snapshot_documents",
        "sqb_snapshot_changes",
        "sqb_snapshots",
    ] {
        (transaction, _) = query(
            transaction,
            &format!("DELETE FROM {table} WHERE snapshot_id IN ({expired_list})"),
            ids.clone(),
        )
        .await?;
    }
    for (((table, column), content, elsewhere), hashes) in CONTENT.iter().zip(hashes) {
        for chunk in hashes.chunks(CONTENT_HASH_CHUNK) {
            let placeholders = Parameter::placeholders(1..=chunk.len());
            (transaction, _) = query(
                transaction,
                &format!(
                    "DELETE FROM {content} WHERE {column} IN ({placeholders}) AND NOT EXISTS \
                     (SELECT 1 FROM {table} n WHERE n.{column}={content}.{column}){elsewhere}"
                ),
                chunk.iter().cloned().map(Parameter::Text).collect(),
            )
            .await?;
        }
    }
    Ok(transaction)
}
