//! Real slot accounting and successful completion through the public worker lifecycle.

use std::collections::BTreeMap;
use std::num::NonZeroU32;

use crate::runs::main::change_run::change_run;
use crate::runs::main::claim_next_run::claim_next_run;
use crate::runs::models::{
    NodeOutcome, RunAuthority, RunChange, RunChangeRequest, RunClaim, RunOutcome,
};
use crate::scheduler::models::SchedulerLease;
use crate::store::models::ControlStore;
use uuid::Uuid;

pub(crate) async fn slot_counts(store: &ControlStore) -> BTreeMap<String, i64> {
    let sql = "SELECT group_key, count(*) FROM sqb_concurrency_slots GROUP BY group_key";
    let ControlStore::Postgres(pool) = store else {
        let ControlStore::Sqlite(path) = store else {
            unreachable!()
        };
        return rusqlite::Connection::open(path)
            .expect("inspect SQLite")
            .prepare(sql)
            .expect("slot query")
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .expect("SQLite slot counts")
            .map(|row| row.map(by_group))
            .collect::<Result<_, _>>()
            .expect("decode slot counts");
    };
    sqlx_core::query_as::query_as::<_, (String, i64)>(sql)
        .fetch_all(pool)
        .await
        .expect("PostgreSQL slot counts")
        .into_iter()
        .map(by_group)
        .collect()
}

/// Slot keys carry their location and target; tests count by group name.
fn by_group((key, count): (String, i64)) -> (String, i64) {
    (
        key.rsplit_once('/')
            .map_or(key.as_str(), |(_, group)| group)
            .to_owned(),
        count,
    )
}

pub(crate) async fn succeed(store: &ControlStore, claim: &RunClaim) {
    finish(store, claim, (NodeOutcome::Success, RunOutcome::Success)).await;
}

/// Every selected node and the run end with the given outcomes.
pub(crate) async fn finish(
    store: &ControlStore,
    claim: &RunClaim,
    (node_outcome, run_outcome): (NodeOutcome, RunOutcome),
) {
    let mut changes = vec![RunChange::Started];
    changes.extend(
        claim
            .request
            .selection
            .iter()
            .map(|node| RunChange::NodeFinished {
                node_key: node.clone(),
                outcome: node_outcome,
                rows_affected: Some(1),
                error_code: None,
                skipped_because_of: None,
                error_message: None,
            }),
    );
    changes.push(RunChange::Finished {
        outcome: run_outcome,
        reason: "synthetic child completed".into(),
        exit_code: Some(0),
    });
    for change in changes {
        change_run(
            store,
            RunAuthority::Worker {
                run_id: claim.request.run_id,
                claim_id: claim.claim_id,
            },
            &RunChangeRequest {
                event_id: Uuid::now_v7(),
                run_id: claim.request.run_id,
                claim_id: Some(claim.claim_id),
                change,
            },
        )
        .await
        .expect("worker fact");
    }
}

/// Claim `count` runs with `caps`, each of which must be claimable, then refuse one more.
pub(crate) async fn claim_exactly(
    store: &ControlStore,
    lease: &SchedulerLease,
    (caps, count): (&BTreeMap<String, NonZeroU32>, usize),
) -> Vec<RunClaim> {
    let mut claims: Vec<RunClaim> = Vec::new();
    for _ in 0..count {
        claims.push(
            claim_next_run(store, lease, caps, None)
                .await
                .expect("claim")
                .expect("claimable within the limit"),
        );
    }
    assert!(
        claim_next_run(store, lease, caps, None)
            .await
            .expect("claim")
            .is_none(),
        "no run past the limit is claimable"
    );
    claims
}
