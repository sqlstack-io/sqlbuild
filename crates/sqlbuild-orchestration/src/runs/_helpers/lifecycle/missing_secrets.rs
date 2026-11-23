//! The nodes a run couldn't run for a missing secret say so in its node list, so a page needs no read per failed node.

use std::collections::BTreeMap;

use uuid::Uuid;

use crate::runs::_helpers::lifecycle::error_text::finished_error_message;
use crate::runs::constants::{
    MISSING_SECRET_ERROR_CODE, MISSING_SECRET_PHRASE, MISSING_SECRET_PREFIX,
};
use crate::runs::models::RunNodeRecord;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

/// `nodes` with the worker's message on each that failed unstarted for a missing secret; any other error text stays behind view.evidence, and an unreadable fact leaves its node without one. The run's invocation bounds the event join, as PostgreSQL has no index on an event's `storage_id`.
pub(in crate::runs) async fn with_missing_secrets(
    transaction: ControlTransaction,
    run_id: Uuid,
    mut nodes: Vec<RunNodeRecord>,
) -> Result<(ControlTransaction, Vec<RunNodeRecord>), ControlError> {
    let possible: bool = nodes.iter().any(|node| {
        node.started_at.is_none() && node.error_code.as_deref() == Some(MISSING_SECRET_ERROR_CODE)
    });
    if !possible {
        return Ok((transaction, nodes));
    }
    let (transaction, rows) = query(
        transaction,
        "SELECT n.node_key, e.payload FROM sqb_run_nodes n \
         JOIN recall_events e ON e.storage_id=n.finished_storage_id \
         WHERE n.run_id=$1 AND e.invocation_id=$2 AND n.kind='node' AND n.status='failed' \
         AND n.error_code=$3 AND n.started_at IS NULL",
        vec![
            Parameter::Id(run_id),
            Parameter::Text(format!("scheduler/{run_id}")),
            Parameter::Text(MISSING_SECRET_ERROR_CODE.into()),
        ],
    )
    .await?;
    let mut messages: BTreeMap<String, String> = BTreeMap::new();
    for row in rows {
        let (Some(key), Ok(Some(message))) = (
            row["node_key"].as_str(),
            finished_error_message(&row["payload"]),
        ) else {
            continue;
        };
        if message.starts_with(MISSING_SECRET_PREFIX) && message.contains(MISSING_SECRET_PHRASE) {
            messages.insert(key.to_owned(), message);
        }
    }
    for node in &mut nodes {
        node.missing_secret = messages.remove(&node.node_key);
    }
    Ok((transaction, nodes))
}
