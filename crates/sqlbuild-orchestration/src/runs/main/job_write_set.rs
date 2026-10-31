//! Resolve physical writers from the pinned snapshot and its published job membership.

use std::collections::BTreeSet;

use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn job_write_set(
    transaction: ControlTransaction,
    snapshot_id: Uuid,
    selection: &BTreeSet<String>,
) -> Result<(ControlTransaction, BTreeSet<String>), ControlError> {
    let (transaction, rows) = query(transaction,
        "SELECT node_key FROM sqb_snapshot_nodes WHERE snapshot_id=$1 AND node_kind IN ('model','seed','udf','table_fn')",
        vec![Parameter::Id(snapshot_id)]).await?;
    Ok((
        transaction,
        rows.into_iter()
            .filter_map(|row| {
                row.get("node_key")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
            })
            .filter(|key| selection.contains(key))
            .collect(),
    ))
}
