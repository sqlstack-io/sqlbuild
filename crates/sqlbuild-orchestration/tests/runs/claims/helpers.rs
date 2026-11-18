//! Real slot accounting and successful completion through the public worker lifecycle.

use std::collections::BTreeMap;
use std::num::NonZeroU32;

use crate::runs::main::execution::change_run::change_run;
use crate::runs::main::execution::claim_next_run::claim_next_run;
use crate::runs::main::execution::publish_run_recall::publish_run_recall;
use crate::runs::models::{
    NodeOutcome, RecallBatch, RecallRecord, RunAuthority, RunChange, RunChangeRequest, RunClaim,
    RunOutcome,
};
use crate::scheduler::models::SchedulerLease;
use crate::store::models::ControlStore;
use serde_json::json;
use sqlbuild_recall::records::models::RecallValue;
use sqlbuild_recall::schema::models::Table;
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

/// Every selected node and the run succeed, the run having let `ignored` audit failures through.
pub(crate) async fn succeed_ignoring_audit_failures(
    store: &ControlStore,
    claim: &RunClaim,
    ignored: u32,
) {
    finish_reporting(
        store,
        claim,
        (NodeOutcome::Success, RunOutcome::Success),
        vec![RunChange::AuditFailuresIgnored { count: ignored }],
    )
    .await;
}

/// The claimed worker records one `audit_completed` result per `(audit, ignored)` on the claim's `orders` model.
pub(crate) async fn publish_audit_results(
    store: &ControlStore,
    claim: &RunClaim,
    audits: &[(String, bool)],
) {
    let records = audits
        .iter()
        .enumerate()
        .map(|(index, (audit, ignored))| {
            let severity = ["warn", "error"][usize::from(*ignored)];
            let payload = json!({"audit_name": audit, "audit_definition_name": "not_null",
                "evaluation_mode": "violations", "outcome": severity, "severity": severity,
                "run_scope_phase": "model", "attachment_kind": "column", "attached_target_kind": "model",
                "attached_target_name": "orders", "attached_column_name": "order_id",
                "binding_key": format!("orders.{audit}"), "violation_count": index + 2,
                "executed_sql": "select 1", "ignored": ignored});
            RecallRecord {
                table: Table::Events,
                values: BTreeMap::from([
                    ("event_id".into(), RecallValue::Text(format!("audit-{audit}"))),
                    ("event_type".into(), RecallValue::Text("audit_completed".into())),
                    ("event_schema_version".into(), RecallValue::Integer(1)),
                    ("event_kind".into(), RecallValue::Text("lifecycle".into())),
                    ("severity".into(), RecallValue::Text("info".into())),
                    (
                        "occurred_at".into(),
                        RecallValue::Text(format!("2026-01-01T00:00:{index:02}Z")),
                    ),
                    ("producer".into(), RecallValue::Text("sqb".into())),
                    ("run_id".into(), RecallValue::Text("sqb-run".into())),
                    (
                        "invocation_sequence".into(),
                        RecallValue::Integer(i64::try_from(index).expect("small index")),
                    ),
                    ("payload".into(), RecallValue::Text(payload.to_string())),
                ]),
            }
        })
        .collect();
    publish_run_recall(
        store,
        (claim.request.run_id, claim.claim_id),
        &RecallBatch {
            invocation_id: "execution".into(),
            records,
        },
    )
    .await
    .expect("publish audit results");
}

/// Every selected node and the run end with the given outcomes.
pub(crate) async fn finish(
    store: &ControlStore,
    claim: &RunClaim,
    outcomes: (NodeOutcome, RunOutcome),
) {
    finish_reporting(store, claim, outcomes, Vec::new()).await;
}

/// `finish`, publishing `reported` after the nodes and before the run finishes.
async fn finish_reporting(
    store: &ControlStore,
    claim: &RunClaim,
    (node_outcome, run_outcome): (NodeOutcome, RunOutcome),
    reported: Vec<RunChange>,
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
    changes.extend(reported);
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

/// Publish the orders location's nightly group: staging, then marts and inventory reading customers.
pub(crate) async fn publish_dependency_group(
    store: &ControlStore,
    location: uuid::Uuid,
    dependency_failure: &serde_json::Value,
) {
    let mut input = crate::runs::tests::helpers::snapshot(location);
    for node in ["orders", "inventory"] {
        input.nodes.get_mut(node).expect("node").definition["deps"] =
            serde_json::json!(["customers"]);
    }
    input.documents.insert(
        "jobs".into(),
        serde_json::json!({
            "staging_job": {"description": "Stage customers", "selection": ["customers"]},
            "marts_job": {"description": "Build orders", "selection": ["orders"]},
            "inventory_job": {"description": "Build inventory", "selection": ["inventory"]}
        }),
    );
    input.documents.insert("groups".into(), serde_json::json!({"nightly_job_group": {
        "description": "Stage, then build orders and inventory",
        "selection": ["customers", "inventory", "orders"],
        "jobs": {"staging_job": [], "marts_job": ["staging_job"], "inventory_job": ["staging_job"]},
        "dependency_failure": dependency_failure
    }}));
    crate::snapshots::main::publication::publish_snapshot::publish_snapshot(store, &input)
        .await
        .expect("publish");
}

/// Run each dependent the failed staging job left runnable, then read the ones the group ended.
pub(crate) async fn member_reactions(
    store: &ControlStore,
    lease: &crate::scheduler::models::SchedulerLease,
    (limits, scope): (
        &std::collections::BTreeMap<String, std::num::NonZeroU32>,
        (uuid::Uuid, &str),
    ),
) -> std::collections::BTreeMap<String, crate::runs::tests::test_types::MemberReaction> {
    use crate::runs::tests::test_types::MemberReaction;
    let mut reactions = std::collections::BTreeMap::new();
    for _ in 0..2 {
        let Some(claim) = crate::runs::main::execution::claim_next_run::claim_next_run(
            store,
            lease,
            limits,
            Some(scope),
        )
        .await
        .expect("claim dependent") else {
            break;
        };
        let execution = crate::runs::main::execution::run_execution::run_execution(
            store,
            (claim.request.run_id, claim.claim_id),
        )
        .await
        .expect("execution")
        .expect("claimed");
        let mut skipped: Vec<(&'static str, &'static str)> = execution
            .skipped_nodes
            .iter()
            .map(|(node, cause)| {
                (
                    ["orders", "inventory"]
                        .into_iter()
                        .find(|known| known == node)
                        .expect("known node"),
                    ["customers"]
                        .into_iter()
                        .find(|known| known == cause)
                        .unwrap_or("other"),
                )
            })
            .collect();
        skipped.sort_unstable();
        let outcome = [
            (NodeOutcome::Skipped, RunOutcome::Failed),
            (NodeOutcome::Success, RunOutcome::Success),
        ][usize::from(skipped.is_empty())];
        finish(store, &claim, outcome).await;
        reactions.insert(
            claim.request.job_key.clone().expect("member"),
            MemberReaction::Runs(skipped),
        );
    }
    let runs = crate::runs::main::listings::list_runs::list_runs(store, scope, 100)
        .await
        .expect("runs");
    let unclaimed: Vec<&str> = ["marts_job", "inventory_job"]
        .into_iter()
        .filter(|member| !reactions.contains_key(*member))
        .collect();
    for member in unclaimed {
        let record = runs
            .iter()
            .find(|record| record.job_key.as_deref() == Some(member))
            .expect("member run");
        assert_eq!(
            (record.status, record.launched_at),
            (crate::runs::models::RunStatus::Skipped, None),
            "{member} ends skipped without launching"
        );
        let nodes = crate::runs::tests::helpers::rows(
            store,
            "SELECT status, skipped_because_of FROM sqb_run_nodes WHERE run_id=$1 AND kind='node'",
            vec![crate::store::models::Parameter::Id(record.run_id)],
        )
        .await;
        assert!(
            nodes.iter().all(
                |node| node["status"] == "skipped" && node["skipped_because_of"] == "customers"
            ),
            "{member}: {nodes:?}"
        );
        reactions.insert(member.into(), MemberReaction::Skipped);
    }
    reactions
}

/// The lease, pool limits and `(location, target)` a test claims runs with.
type ClaimScope<'a> = (
    &'a SchedulerLease,
    (&'a BTreeMap<String, NonZeroU32>, (Uuid, &'a str)),
);

/// With a retry delay, nothing starts until the test moves the retry's `not_before` into the past.
pub(crate) async fn wait_out_retry_delay(
    store: &ControlStore,
    (lease, (limits, scope)): ClaimScope<'_>,
    (run_id, delay_seconds): (Uuid, u32),
    description: &str,
) {
    let 1.. = delay_seconds else {
        return;
    };
    assert!(
        claim_next_run(store, lease, limits, Some(scope))
            .await
            .expect("claim during the delay")
            .is_none(),
        "{description}: nothing starts while the member waits out its delay"
    );
    let stored = crate::runs::tests::helpers::rows(
        store,
        "SELECT request_json FROM sqb_runs WHERE run_id=$1",
        vec![crate::store::models::Parameter::Id(run_id)],
    )
    .await
    .remove(0)["request_json"]
        .clone();
    let mut request = stored.as_str().map_or(stored.clone(), |text| {
        serde_json::from_str(text).expect("request")
    });
    request["retry"]["not_before"] = serde_json::json!("2000-01-01T00:00:00Z");
    let transaction =
        crate::store::main::begin::begin(store, crate::store::models::TransactionMode::Write)
            .await
            .expect("begin");
    let (transaction, _) = crate::store::main::query::query(
        transaction,
        "UPDATE sqb_runs SET request_json=$1 WHERE run_id=$2",
        vec![
            crate::store::models::Parameter::Json(request),
            crate::store::models::Parameter::Id(run_id),
        ],
    )
    .await
    .expect("end the delay");
    crate::store::main::commit::commit(transaction)
        .await
        .expect("commit");
}
