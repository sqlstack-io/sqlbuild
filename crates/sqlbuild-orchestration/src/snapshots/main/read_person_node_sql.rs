//! Live SQL permission, current publication and sensitive-read audit share one writer fence.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use uuid::Uuid;

use crate::authorization::main::begin_authorized::begin_authorized;
use crate::credentials::main::audit::append_audit::append_audit;
use crate::credentials::models::{Action, AuditContext, DefaultRole, PermissionScope};
use crate::snapshots::_helpers::reading::attach_sources;
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
                (transaction, documents) =
                    read_snapshot_documents(transaction, record.snapshot_id, Some(&["columns"]))
                        .await?;
                let expression_ranges = expression_ranges(
                    documents
                        .get("columns")
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

/// Each column's expression range from the node's `columns` document entry; a column whose
/// range is missing, malformed or off the compiled SQL's character boundaries has none.
fn expression_ranges(columns: Option<&Value>, compiled_sql: &str) -> BTreeMap<String, [usize; 2]> {
    let Some(columns) = columns.and_then(Value::as_object) else {
        return BTreeMap::new();
    };
    columns
        .iter()
        .filter_map(|(column, metadata)| {
            let range = metadata.get("expression_range")?.as_array()?;
            let [start, end] = range.as_slice() else {
                return None;
            };
            let start = usize::try_from(start.as_u64()?).ok()?;
            let end = usize::try_from(end.as_u64()?).ok()?;
            compiled_sql
                .get(start..end)
                .filter(|text| !text.is_empty())
                .map(|_| (column.clone(), [start, end]))
        })
        .collect()
}
