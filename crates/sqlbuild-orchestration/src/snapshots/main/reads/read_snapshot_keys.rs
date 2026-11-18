//! Read a snapshot's node keys without loading any definition.

use std::collections::BTreeSet;

use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn read_snapshot_keys(
    transaction: ControlTransaction,
    snapshot_id: Uuid,
) -> Result<(ControlTransaction, BTreeSet<String>), ControlError> {
    let (transaction, rows) = query(
        transaction,
        "SELECT node_key FROM sqb_snapshot_nodes WHERE snapshot_id=$1",
        vec![Parameter::Id(snapshot_id)],
    )
    .await?;
    let keys: BTreeSet<String> = rows
        .iter()
        .filter_map(|row| row["node_key"].as_str().map(str::to_owned))
        .collect();
    Ok((transaction, keys))
}
