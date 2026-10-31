//! Real slot accounting and successful completion through the public worker lifecycle.

use std::collections::BTreeMap;

use sqlbuild_orchestration::runs::main::change_run::change_run;
use sqlbuild_orchestration::runs::models::{
    NodeOutcome, RunAuthority, RunChange, RunChangeRequest, RunClaim, RunOutcome,
};
use sqlbuild_orchestration::store::models::ControlStore;
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
            .collect::<Result<_, _>>()
            .expect("decode slot counts");
    };
    sqlx_core::query_as::query_as::<_, (String, i64)>(sql)
        .fetch_all(pool)
        .await
        .expect("PostgreSQL slot counts")
        .into_iter()
        .collect()
}

pub(crate) async fn succeed(store: &ControlStore, claim: &RunClaim) {
    let mut changes = vec![RunChange::Started];
    changes.extend(
        claim
            .request
            .selection
            .iter()
            .map(|node| RunChange::NodeFinished {
                node_key: node.clone(),
                outcome: NodeOutcome::Success,
                rows_affected: Some(1),
                error_code: None,
                skipped_because_of: None,
            }),
    );
    changes.push(RunChange::Finished {
        outcome: RunOutcome::Success,
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
