//! Immutable body inserts shared by snapshot publication.

use std::collections::{BTreeMap, BTreeSet};

use uuid::Uuid;

use crate::snapshots::models::{PreparedNode, SnapshotChange};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn store_nodes(
    mut transaction: ControlTransaction,
    snapshot_id: Uuid,
    nodes: BTreeMap<String, PreparedNode>,
) -> Result<ControlTransaction, ControlError> {
    for (key, node) in nodes {
        (transaction, _) = query(transaction,
            "INSERT INTO sqb_node_definitions (node_hash, body) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            vec![Parameter::Text(node.hash.clone()), Parameter::Bytes(node.definition)]).await?;
        (transaction, _) = query(transaction,
            "INSERT INTO sqb_snapshot_nodes (snapshot_id, node_key, node_kind, node_hash) VALUES ($1, $2, $3, $4)",
            vec![Parameter::Id(snapshot_id), Parameter::Text(key.clone()), Parameter::Text(node.kind), Parameter::Text(node.hash)]).await?;
        for (kind, (hash, body)) in node.sources {
            (transaction, _) = query(transaction,
                "INSERT INTO sqb_source_texts (text_hash, body) VALUES ($1, $2) ON CONFLICT DO NOTHING",
                vec![Parameter::Text(hash.clone()), Parameter::Bytes(body)]).await?;
            (transaction, _) = query(transaction,
                "INSERT INTO sqb_snapshot_sources (snapshot_id, node_key, kind, text_hash) VALUES ($1, $2, $3, $4)",
                vec![Parameter::Id(snapshot_id), Parameter::Text(key.clone()), Parameter::Text(kind.into()), Parameter::Text(hash)]).await?;
        }
    }
    Ok(transaction)
}

pub(crate) async fn store_changes(
    mut transaction: ControlTransaction,
    snapshot_id: Uuid,
    previous: &BTreeMap<String, String>,
    hashes: &BTreeMap<String, String>,
) -> Result<ControlTransaction, ControlError> {
    let keys: BTreeSet<&String> = previous.keys().chain(hashes.keys()).collect();
    let mut sequence = 0;
    for key in keys {
        let before = previous.get(key);
        let after = hashes.get(key);
        if before == after {
            continue;
        }
        let change = SnapshotChange {
            change_kind: match (before, after) {
                (None, _) => "added",
                (_, None) => "removed",
                _ => "changed",
            }
            .into(),
            subject_key: key.clone(),
            before_hash: before.cloned(),
            after_hash: after.cloned(),
        };
        (transaction, _) = query(transaction,
            "INSERT INTO sqb_snapshot_changes (snapshot_id, change_seq, change_kind, subject_key, before_hash, after_hash) \
             VALUES ($1,$2,$3,$4,$5,$6)", vec![Parameter::Id(snapshot_id), Parameter::Integer(sequence),
                Parameter::Text(change.change_kind), Parameter::Text(change.subject_key),
                Parameter::OptionalText(change.before_hash), Parameter::OptionalText(change.after_hash)]).await?;
        sequence += 1;
    }
    Ok(transaction)
}
