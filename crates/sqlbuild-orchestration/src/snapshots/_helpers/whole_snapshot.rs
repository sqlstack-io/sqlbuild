//! Whole-snapshot reads: decoded once per snapshot, in pages read by short transactions, within a shared memory budget.

use std::collections::BTreeMap;
use std::sync::Arc;

use tokio::sync::OnceCell;
use uuid::Uuid;

use crate::snapshots::_helpers::reading::read_nodes;
use crate::snapshots::constants::{
    DECODED_BODY_EXPANSION, SNAPSHOT_DECODE_BUDGET_MIB, SNAPSHOT_NODE_CACHE_ENTRIES,
    SNAPSHOT_NODE_PAGE,
};
use crate::snapshots::main::reads::snapshot_cell::snapshot_cell;
use crate::snapshots::models::{SnapshotNode, SnapshotNodeCache};
use crate::snapshots::types::SnapshotNodes;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// Every store's decoded snapshots; snapshot IDs are unique, so stores never share an entry.
pub(crate) static SNAPSHOT_NODES: SnapshotNodeCache = SnapshotNodeCache::new();

const MIB: u64 = 1024 * 1024;

/// The snapshot's cell, marked most recently used; adding one drops the least recently used beyond the limit.
pub(crate) fn slot(cache: &SnapshotNodeCache, snapshot_id: Uuid) -> Arc<OnceCell<SnapshotNodes>> {
    snapshot_cell(&cache.slots, ((), snapshot_id), SNAPSHOT_NODE_CACHE_ENTRIES)
}

/// Every node of the snapshot, decoded within `cache`'s budget: it waits until its estimate fits.
pub(crate) async fn read_budgeted(
    cache: &SnapshotNodeCache,
    store: &ControlStore,
    snapshot_id: Uuid,
) -> Result<BTreeMap<String, SnapshotNode>, ControlError> {
    let (count, compressed) = stored_size(store, snapshot_id).await?;
    let estimate = compressed
        .saturating_mul(DECODED_BODY_EXPANSION)
        .div_ceil(MIB);
    let permits = u32::try_from(estimate)
        .unwrap_or(u32::MAX)
        .clamp(1, SNAPSHOT_DECODE_BUDGET_MIB);
    let _reserved = cache
        .budget
        .acquire_many(permits)
        .await
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    let mut nodes: BTreeMap<String, SnapshotNode> = BTreeMap::new();
    let mut after: Option<String> = None;
    loop {
        let rows = node_page(store, snapshot_id, after.as_deref()).await?;
        let full = rows.len() == SNAPSHOT_NODE_PAGE;
        let page = tokio::task::spawn_blocking(move || read_nodes(rows)).await??;
        after = page.keys().next_back().cloned();
        nodes.extend(page);
        if !full || after.is_none() {
            break;
        }
    }
    if nodes.len() != count {
        return Err(ControlError::Invalid(format!(
            "snapshot {snapshot_id} changed while it was read"
        )));
    }
    Ok(nodes)
}

/// How many nodes the snapshot has and their definitions' compressed bytes.
async fn stored_size(
    store: &ControlStore,
    snapshot_id: Uuid,
) -> Result<(usize, u64), ControlError> {
    let transaction = begin(store, TransactionMode::Read).await?;
    let length = match store {
        ControlStore::Postgres(_) => "octet_length",
        ControlStore::Sqlite(_) => "length",
    };
    let (transaction, rows) = query(
        transaction,
        &format!(
            "SELECT COUNT(*) AS nodes, COALESCE(SUM({length}(d.body)), 0) AS bytes \
             FROM sqb_snapshot_nodes n JOIN sqb_node_definitions d ON d.node_hash=n.node_hash \
             WHERE n.snapshot_id=$1"
        ),
        vec![Parameter::Id(snapshot_id)],
    )
    .await?;
    commit(transaction).await?;
    let number = |name: &str| {
        rows.first()
            .and_then(|row| row.get(name))
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| {
                ControlError::Invalid(format!("snapshot {snapshot_id} has no {name} count"))
            })
    };
    let nodes = usize::try_from(number("nodes")?)
        .map_err(|error| ControlError::Invalid(format!("snapshot node count: {error}")))?;
    Ok((nodes, number("bytes")?))
}

/// The next page of the snapshot's nodes after `after`, by key, read and committed before anything is decoded.
async fn node_page(
    store: &ControlStore,
    snapshot_id: Uuid,
    after: Option<&str>,
) -> Result<Vec<crate::store::types::Row>, ControlError> {
    let transaction = begin(store, TransactionMode::Read).await?;
    let mut parameters = vec![Parameter::Id(snapshot_id)];
    let cursor = match after {
        Some(after) => {
            parameters.push(Parameter::Text(after.to_owned()));
            " AND n.node_key>$2"
        }
        None => "",
    };
    let select = format!(
        "SELECT n.node_key, {} AS body FROM sqb_snapshot_nodes n JOIN sqb_node_definitions d \
         ON d.node_hash=n.node_hash WHERE n.snapshot_id=$1{cursor} ORDER BY n.node_key LIMIT {SNAPSHOT_NODE_PAGE}",
        transaction.hex_column("d.body"),
    );
    let (transaction, rows) = query(transaction, &select, parameters).await?;
    commit(transaction).await?;
    Ok(rows)
}
