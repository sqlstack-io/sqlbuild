//! Each node's newest decisive scheduler attempt, read by an indexed seek per node.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::query_records::query_records;
use crate::store::models::{ControlTransaction, Parameter};
use crate::warehouse_metadata::models::{CatalogNodeBuild, CatalogNodeOutcome};

/// Node keys per statement; each takes one bound parameter.
const NODES_PER_STATEMENT: usize = 1_000;

#[derive(Deserialize)]
struct Attempt {
    node_key: String,
    run_id: Uuid,
    status: String,
    ended_at: Option<DateTime<Utc>>,
}

/// The newest started `success` or `failed` attempt of each node by a non-group run in this target.
pub(crate) async fn latest_attempts(
    mut transaction: ControlTransaction,
    (location_id, target, node_keys): (Uuid, &str, &[&str]),
) -> Result<(ControlTransaction, HashMap<String, CatalogNodeOutcome>), ControlError> {
    let mut outcomes: HashMap<String, CatalogNodeOutcome> = HashMap::new();
    for chunk in node_keys.chunks(NODES_PER_STATEMENT) {
        let mut parameters = vec![Parameter::Id(location_id), Parameter::Text(target.into())];
        let mut rows: Vec<String> = Vec::with_capacity(chunk.len());
        for key in chunk {
            parameters.push(Parameter::Text((*key).into()));
            rows.push(format!("(${})", parameters.len()));
        }
        let attempts: Vec<Attempt>;
        (transaction, attempts) = query_records(transaction,
            &format!("SELECT n.node_key,n.run_id,n.status,n.ended_at FROM (VALUES {}) AS w JOIN sqb_run_nodes n \
                ON n.node_key=w.column1 AND n.run_id=(SELECT m.run_id FROM sqb_run_nodes m JOIN sqb_runs r ON r.run_id=m.run_id \
                WHERE m.node_key=w.column1 AND m.finished_storage_id IS NOT NULL AND m.status IN ('success','failed') \
                AND m.started_at IS NOT NULL AND r.location_id=$1 AND r.target=$2 AND r.kind<>'group' \
                ORDER BY m.finished_storage_id DESC LIMIT 1)", rows.join(",")),
            parameters).await?;
        outcomes.extend(attempts.into_iter().filter_map(|attempt| {
            Some((
                attempt.node_key,
                CatalogNodeOutcome {
                    status: attempt.status,
                    at: attempt.ended_at?,
                    run_id: Some(attempt.run_id),
                },
            ))
        }));
    }
    Ok((transaction, outcomes))
}

/// The scheduler attempt, unless a build outside the scheduler recorded a fingerprint after it ended.
pub(crate) fn last_outcome(
    attempt: Option<CatalogNodeOutcome>,
    build: Option<&CatalogNodeBuild>,
) -> Option<CatalogNodeOutcome> {
    let newer_build =
        build.filter(|build| attempt.as_ref().is_none_or(|a| build.observed_at > a.at));
    match newer_build {
        Some(build) => Some(CatalogNodeOutcome {
            status: "success".into(),
            at: build.observed_at,
            run_id: None,
        }),
        None => attempt,
    }
}
