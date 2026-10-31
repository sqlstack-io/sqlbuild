//! Resolve physical writers from the pinned snapshot and its published job membership.

use std::collections::BTreeSet;

use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn job_write_set(
    transaction: ControlTransaction,
    snapshot_id: Uuid,
    (location_id, target, job_key): (Uuid, &str, &str),
) -> Result<(ControlTransaction, BTreeSet<String>), ControlError> {
    let (transaction, rows) = query(transaction,
        "SELECT n.node_key FROM sqb_snapshot_nodes n JOIN sqb_job_nodes j ON j.node_key=n.node_key \
         WHERE n.snapshot_id=$1 AND j.location_id=$2 AND j.target=$3 AND j.job_key=$4 AND n.node_kind IN ('model','seed','udf','table_fn')",
        vec![Parameter::Id(snapshot_id), Parameter::Id(location_id), Parameter::Text(target.into()), Parameter::Text(job_key.into())]).await?;
    Ok((
        transaction,
        rows.into_iter()
            .filter_map(|row| {
                row.get("node_key")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
            })
            .collect(),
    ))
}
