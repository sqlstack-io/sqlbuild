//! Live SQL permission, current publication and sensitive-read audit share one writer fence.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use uuid::Uuid;

use crate::authorization::main::begin_authorized::begin_authorized;
use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::models::{Action, AuditContext, DefaultRole, PermissionScope};
use crate::snapshots::_helpers::reading::attach_sources;
use crate::snapshots::constants::COLUMNS_DOCUMENT;
use crate::snapshots::main::read_current_record::read_current_record;
use crate::snapshots::main::read_snapshot_documents::read_snapshot_documents;
use crate::snapshots::main::read_snapshot_nodes::read_snapshot_nodes;
use crate::snapshots::models::NodeSql;
use crate::store::errors::ControlError;
use crate::store::main::{commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter};

pub async fn read_person_node_sql(
    store: &ControlStore,
    credentials: (&AuditContext, Uuid),
    (location_id, target, node_key): (Uuid, &str, &str),
) -> Result<Option<NodeSql>, ControlError> {
    let scope = PermissionScope {
        action: Action::ViewSql,
        location_id: Some(location_id),
        target: Some(target.into()),
        nodes: [node_key.into()].into(),
    };
    let (transaction, context) =
        begin_authorized(store, credentials, &scope, DefaultRole::None).await?;
    let (mut transaction, record) = read_current_record(transaction, (location_id, target)).await?;
    let mut result = None;
    if let Some(record) = record {
        let nodes;
        (transaction, nodes) =
            read_snapshot_nodes(transaction, record.snapshot_id, Some(&scope.nodes)).await?;
        if !nodes.is_empty() {
            let rows;
            (transaction, rows) = query(transaction,
                "SELECT s.node_key, s.kind, t.body FROM sqb_snapshot_sources s JOIN sqb_source_texts t ON t.text_hash=s.text_hash \
                 WHERE s.snapshot_id=$1 AND s.node_key=$2 AND s.kind IN ('raw_sql','compiled_sql')",
                vec![Parameter::Id(record.snapshot_id), Parameter::Text(node_key.into())]).await?;
            if let Some(node) = attach_sources(nodes, rows)?.remove(node_key) {
                let documents;
                (transaction, documents) = read_snapshot_documents(
                    transaction,
                    record.snapshot_id,
                    Some(&[COLUMNS_DOCUMENT]),
                )
                .await?;
                let expression_ranges = expression_ranges(
                    documents
                        .get(COLUMNS_DOCUMENT)
                        .and_then(|columns| columns.get(node_key)),
                    node.compiled_sql.as_deref().unwrap_or_default(),
                );
                result = Some(NodeSql {
                    snapshot_id: record.snapshot_id,
                    node_key: node_key.into(),
                    raw_sql: node.raw_sql,
                    compiled_sql: node.compiled_sql,
                    expression_ranges,
                });
            }
        }
    }
    let transaction = append_audit(
        transaction,
        &context,
        (Uuid::now_v7(), "sql.read"),
        (json!(scope), json!(null), json!({"snapshot_id":result.as_ref().map(|node| node.snapshot_id),"found":result.is_some()})),
    )
    .await?;
    commit(transaction).await?;
    Ok(result)
}

/// Each column's well-formed expression range from the node's `columns` document entry.
fn expression_ranges(columns: Option<&Value>, compiled_sql: &str) -> BTreeMap<String, [usize; 2]> {
    let mut ranges: BTreeMap<String, [usize; 2]> = BTreeMap::new();
    let Some(columns) = columns.and_then(Value::as_object) else {
        return ranges;
    };
    for (column, metadata) in columns {
        if let Some(range) = column_range(metadata, compiled_sql) {
            ranges.insert(column.clone(), range);
        }
    }
    ranges
}

/// A column's `[start, end)` compiled-SQL byte range, or `None` when missing or malformed.
fn column_range(metadata: &Value, compiled_sql: &str) -> Option<[usize; 2]> {
    let Some([start, end]) = metadata
        .get("expression_range")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
    else {
        return None;
    };
    let (start, end): (u64, u64) = (start.as_u64()?, end.as_u64()?);
    let length: u64 = u64::try_from(compiled_sql.len()).unwrap_or(u64::MAX);
    if start >= end || end > length {
        return None;
    }
    let range: [usize; 2] = [start as usize, end as usize];
    compiled_sql.get(range[0]..range[1]).map(|_| range)
}
