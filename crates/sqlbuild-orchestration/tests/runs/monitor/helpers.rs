//! Advance persisted liveness, not the scheduler clock, to exercise real monitoring transactions.

use std::collections::BTreeMap;

use chrono::{TimeDelta, Utc};
use sqlbuild_orchestration::runs::main::change_run::change_run;
use sqlbuild_orchestration::runs::main::claim_next_run::claim_next_run;
use sqlbuild_orchestration::runs::models::{
    RunAuthority, RunChange, RunChangeRequest, RunClaim, RunRequest,
};
use sqlbuild_orchestration::scheduler::models::SchedulerLease;
use sqlbuild_orchestration::store::models::ControlStore;
use uuid::Uuid;

use crate::helpers::queue;

pub(crate) async fn start_run(
    store: &ControlStore,
    lease: &SchedulerLease,
    input: &RunRequest,
) -> RunClaim {
    queue(store, input).await;
    let claim = claim_next_run(store, lease, &BTreeMap::new(), None)
        .await
        .expect("claim queued run")
        .expect("run available");
    assert_eq!(claim.request.run_id, input.run_id);
    change_run(
        store,
        RunAuthority::Worker {
            run_id: input.run_id,
            claim_id: claim.claim_id,
        },
        &RunChangeRequest {
            event_id: Uuid::now_v7(),
            run_id: input.run_id,
            claim_id: Some(claim.claim_id),
            change: RunChange::Started,
        },
    )
    .await
    .expect("start claimed worker");
    claim
}

pub(crate) async fn age_heartbeat(store: &ControlStore, run_id: Uuid, seconds: i64) {
    let timestamp = Utc::now() - TimeDelta::seconds(seconds);
    let ControlStore::Postgres(pool) = store else {
        let ControlStore::Sqlite(path) = store else {
            unreachable!()
        };
        rusqlite::Connection::open(path)
            .expect("open liveness fixture")
            .execute(
                "UPDATE sqb_run_liveness SET last_heartbeat_at=?1 WHERE run_id=?2",
                rusqlite::params![
                    timestamp.to_rfc3339_opts(chrono::SecondsFormat::Micros, true),
                    run_id.to_string()
                ],
            )
            .expect("age SQLite heartbeat");
        return;
    };
    sqlx_core::query::query("UPDATE sqb_run_liveness SET last_heartbeat_at=$1 WHERE run_id=$2")
        .bind(timestamp)
        .bind(run_id)
        .execute(pool)
        .await
        .expect("age PostgreSQL heartbeat");
}
