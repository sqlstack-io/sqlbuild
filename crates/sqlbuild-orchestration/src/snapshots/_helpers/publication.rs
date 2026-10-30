//! Immutable body inserts shared by snapshot publication.

use std::collections::BTreeMap;

use uuid::Uuid;

use crate::snapshots::models::PreparedNode;
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
