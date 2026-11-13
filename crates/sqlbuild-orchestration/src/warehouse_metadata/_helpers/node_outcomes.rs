//! Each node's newest decisive attempt among the target's newest scheduler runs.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::main::query_records::query_records;
use crate::store::models::{ControlTransaction, Parameter};
use crate::warehouse_metadata::models::{CatalogNodeBuild, CatalogNodeOutcome};

/// The target's newest runs whose attempts are consulted, so a listing reads a bounded slice of
/// this target's history and none of other targets'.
const RECENT_RUNS: i64 = 25;

#[derive(Deserialize)]
struct Attempt {
    node_key: String,
    run_id: Uuid,
    status: String,
    ended_at: Option<DateTime<Utc>>,
    finished_storage_id: i64,
}

/// Each node's newest started `success`, `warn` or `failed` attempt in the target's newest non-group runs.
pub(crate) async fn latest_attempts(
    transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
) -> Result<(ControlTransaction, HashMap<String, CatalogNodeOutcome>), ControlError> {
    let (transaction, attempts): (_, Vec<Attempt>) = query_records(transaction,
        "SELECT n.node_key,n.run_id,n.status,n.ended_at,n.finished_storage_id FROM (SELECT run_id FROM sqb_runs \
         WHERE location_id=$1 AND target=$2 AND kind<>'group' ORDER BY queued_at DESC, run_id DESC LIMIT $3) AS r \
         JOIN sqb_run_nodes n ON n.run_id=r.run_id WHERE n.kind='node' AND n.finished_storage_id IS NOT NULL \
         AND n.started_at IS NOT NULL AND n.status IN ('success','warn','failed')",
        vec![Parameter::Id(location_id), Parameter::Text(target.into()), Parameter::Integer(RECENT_RUNS)]).await?;
    let mut newest: HashMap<String, Attempt> = HashMap::new();
    for attempt in attempts {
        if newest
            .get(&attempt.node_key)
            .is_none_or(|seen| attempt.finished_storage_id > seen.finished_storage_id)
        {
            newest.insert(attempt.node_key.clone(), attempt);
        }
    }
    let outcomes = newest
        .into_iter()
        .filter_map(|(key, attempt)| {
            Some((
                key,
                CatalogNodeOutcome {
                    status: attempt.status,
                    at: attempt.ended_at?,
                    run_id: Some(attempt.run_id),
                },
            ))
        })
        .collect();
    Ok((transaction, outcomes))
}

/// The scheduler attempt, unless a build outside the scheduler recorded a fingerprint after it
/// ended. The fingerprint time comes from the building machine's clock and the attempt's end
/// from the control store's, so a skew between them can misorder builds made close together.
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
