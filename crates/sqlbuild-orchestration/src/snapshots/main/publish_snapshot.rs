//! Publish a complete generation and its current pointer in one transaction.

use std::collections::BTreeMap;

use serde_json::Value;
use uuid::Uuid;

use crate::declarations::main::publish_definitions::publish_definitions;
use crate::declarations::main::snapshot_definitions::snapshot_definitions;
use crate::snapshots::_helpers::preparation::prepare_snapshot;
use crate::snapshots::_helpers::publication::{store_changes, store_nodes};
use crate::snapshots::models::{PreparedSnapshot, SnapshotInput, SnapshotRecord};
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// Reject stale generations without changing the last good snapshot.
pub async fn publish_snapshot(
    store: &ControlStore,
    input: &SnapshotInput,
) -> Result<SnapshotRecord, ControlError> {
    let definitions = snapshot_definitions(input)?;
    let PreparedSnapshot {
        nodes,
        hashes,
        documents,
        manifest_digest,
    } = prepare_snapshot(input)?;
    let transaction = begin(store, TransactionMode::Write).await?;
    let (transaction, previous) = query(transaction,
        "SELECT s.generation FROM sqb_current_snapshots c JOIN sqb_snapshots s ON s.snapshot_id=c.snapshot_id \
         WHERE c.location_id=$1 AND c.target=$2",
        vec![Parameter::Id(input.location_id), Parameter::Text(input.target.clone())]).await?;
    if previous
        .first()
        .and_then(|row| row.get("generation"))
        .and_then(Value::as_i64)
        .is_some_and(|generation| generation >= input.generation)
    {
        return Err(ControlError::StaleGeneration);
    }
    let (mut transaction, previous) = query(transaction,
        "SELECT n.node_key, n.node_hash FROM sqb_snapshot_nodes n JOIN sqb_current_snapshots c ON c.snapshot_id=n.snapshot_id \
         WHERE c.location_id=$1 AND c.target=$2",
        vec![Parameter::Id(input.location_id), Parameter::Text(input.target.clone())]).await?;
    let previous: BTreeMap<String, String> = previous
        .into_iter()
        .map(|row| {
            (
                row["node_key"].as_str().unwrap_or_default().into(),
                row["node_hash"].as_str().unwrap_or_default().into(),
            )
        })
        .collect();
    let record = SnapshotRecord {
        snapshot_id: Uuid::now_v7(),
        location_id: input.location_id,
        target: input.target.clone(),
        generation: input.generation,
        manifest_digest,
        published_at: transaction.now,
    };
    (transaction, _) = query(transaction,
        "INSERT INTO sqb_snapshots (snapshot_id, location_id, target, generation, git_revision, git_dirty, git_author, image_id, \
         environment_hash, sqb_version, snapshot_schema_version, manifest_digest, published_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,1,$11,$12)", vec![
            Parameter::Id(record.snapshot_id), Parameter::Id(input.location_id), Parameter::Text(input.target.clone()),
            Parameter::Integer(input.generation), Parameter::OptionalText(input.git_revision.clone()), Parameter::Boolean(input.git_dirty),
            Parameter::OptionalText(input.git_author.clone()), Parameter::OptionalText(input.image_id.clone()),
            Parameter::Text(input.environment_hash.clone()), Parameter::Text(input.sqb_version.clone()),
            Parameter::Text(record.manifest_digest.clone()), Parameter::Time(record.published_at),
        ]).await?;
    transaction = store_nodes(transaction, record.snapshot_id, nodes).await?;
    for (kind, (hash, body)) in documents {
        (transaction, _) = query(transaction,
            "INSERT INTO sqb_documents (document_hash, body) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            vec![Parameter::Text(hash.clone()), Parameter::Bytes(body)]).await?;
        (transaction, _) = query(transaction,
            "INSERT INTO sqb_snapshot_documents (snapshot_id, kind, document_hash) VALUES ($1, $2, $3)",
            vec![Parameter::Id(record.snapshot_id), Parameter::Text(kind), Parameter::Text(hash)]).await?;
    }
    transaction = store_changes(transaction, record.snapshot_id, &previous, &hashes).await?;
    (transaction, _) = query(transaction,
        "INSERT INTO sqb_current_snapshots (location_id, target, snapshot_id, since) VALUES ($1,$2,$3,$4) \
         ON CONFLICT (location_id, target) DO UPDATE SET snapshot_id=excluded.snapshot_id, since=excluded.since",
        vec![Parameter::Id(input.location_id), Parameter::Text(input.target.clone()),
            Parameter::Id(record.snapshot_id), Parameter::Time(record.published_at)]).await?;
    transaction = publish_definitions(transaction, &record, definitions).await?;
    commit(transaction).await?;
    Ok(record)
}
