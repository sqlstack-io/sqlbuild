//! Run log retention deletes whole worker logs by age or count and keeps runs and their history.

use chrono::TimeDelta;
use serde_json::{Value, json};
use sqlbuild_recall::records::main::artifact_rows::artifact_rows;
use sqlbuild_recall::records::main::event_object_row::event_object_row;
use sqlbuild_recall::records::main::output_row::output_row;
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;
use sqlbuild_recall::records::models::{
    ArtifactInput, OutputChunk, OutputStream, RecallRow, RowEnvelope,
};
use uuid::Uuid;

use crate::credentials::tests::location_helpers::create_location;
use crate::runs::main::prune_run_logs::prune_run_logs;
use crate::runs::models::RunLogRetention;
use crate::runs::tests::helpers::{request, request_run, snapshot};
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::main::append_recall::append_recall;
use crate::store::main::migrate_store::migrate_store;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

const SHARED_SQL: &str = "select order_id from orders";

fn envelope(invocation_id: String) -> RowEnvelope {
    RowEnvelope {
        invocation_id,
        invocation_started_at: "2026-01-02T03:04:05Z".into(),
        producer_version: "test".into(),
    }
}

fn sql_rows(envelope: &RowEnvelope, sql: &str) -> [RecallRow; 2] {
    artifact_rows(
        envelope,
        &ArtifactInput {
            digest: format!("sha256:{}", sha256_hex(sql.as_bytes())),
            kind: "sql".into(),
            media_type: "text/x-sql".into(),
            content: sql.as_bytes().to_vec(),
            seen_at: "2026-01-02T03:04:05Z".into(),
            ref_id: "statement".into(),
            role: "executed_sql".into(),
            statement_id: Some("statement".into()),
            resource_id: None,
        },
    )
}

/// The claim's output, and a nested session's event and SQL: one text shared by every run, one its own.
fn worker_log(claim: Uuid, index: u128) -> Vec<RecallRow> {
    let worker = envelope(format!("worker/{claim}"));
    let session = envelope(format!("worker/{claim}/session"));
    let mut rows = vec![output_row(
        &worker,
        &OutputChunk {
            sequence: 0,
            stream: OutputStream::Stdout,
            occurred_at: "2026-01-02T03:04:05Z".into(),
            data: b"building orders\n".to_vec(),
            byte_offset: 0,
            dropped_before: 0,
            omitted: None,
        },
    )];
    rows.push(
        event_object_row(
            &session,
            json!({"event_id": "completed", "event_type": "statement_completed", "schema_version": 1,
                "occurred_at": "2026-01-02T03:04:05Z", "producer": "sqlbuild", "invocation_sequence": 1,
                "statement_id": "statement", "payload": {"duration_ms": 1.5}})
            .as_object()
            .expect("event object"),
        )
        .expect("statement event"),
    );
    rows.extend(sql_rows(&session, SHARED_SQL));
    rows.extend(sql_rows(&session, &format!("select {index} as order_id")));
    rows
}

async fn read(store: &ControlStore, sql: &str) -> Vec<Value> {
    let transaction = begin(store, TransactionMode::Read).await.expect("begin");
    let (transaction, rows) = query(transaction, sql, Vec::new()).await.expect("query");
    commit(transaction).await.expect("commit");
    rows.into_iter().map(|row| json!(row)).collect()
}

#[test]
fn given_finished_runs_past_age_or_count_when_logs_are_pruned_then_whole_logs_go_and_runs_stay() {
    for backend in [Backend::Sqlite, Backend::Postgres] {
        run(async {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../benchmarks/duckshop/edge_cases/orchestration/run_log_retention.json");
            let case: Value =
                serde_json::from_str(&std::fs::read_to_string(path).expect("run log fixture"))
                    .expect("run log case");
            let Some(fixture) = open_test_store(backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let location = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let published = publish_snapshot(store, &snapshot(location))
                .await
                .expect("publish");
            let ended = case["ended_hours_ago"].as_array().expect("runs");
            let mut transaction = begin(store, TransactionMode::Write).await.expect("begin");
            let now = transaction.now;
            for (position, hours) in ended.iter().enumerate() {
                let index = position as u128 + 1;
                let run_request = request((location, published.snapshot_id), index, 0, &["orders"]);
                let run_id = run_request.run_id;
                commit(transaction).await.expect("commit");
                assert!(request_run(store, &run_request).await.expect("request"));
                transaction = begin(store, TransactionMode::Write).await.expect("begin");
                let claim = Uuid::new_v5(&run_id, b"run_claimed");
                let (sql, ended_at) = match hours.as_i64() {
                    Some(hours) => (
                        "UPDATE sqb_runs SET status='success', worker_instance=$1, ended_at=$2 WHERE run_id=$3",
                        Some(Parameter::Time(now - TimeDelta::hours(hours))),
                    ),
                    None => (
                        "UPDATE sqb_runs SET status='running', worker_instance=$1 WHERE run_id=$2",
                        None,
                    ),
                };
                let parameters: Vec<Parameter> = [
                    Some(Parameter::Id(claim)),
                    ended_at,
                    Some(Parameter::Id(run_id)),
                ]
                .into_iter()
                .flatten()
                .collect();
                (transaction, _) = query(transaction, sql, parameters)
                    .await
                    .expect("finish run");
                let scheduler = event_object_row(
                    &envelope(format!("scheduler/{run_id}")),
                    json!({"event_id": format!("queued-{index}"), "event_type": "run_queued", "schema_version": 1,
                        "occurred_at": "2026-01-02T03:04:05Z", "producer": "scheduler", "invocation_sequence": 1,
                        "payload": {}})
                    .as_object()
                    .expect("event object"),
                )
                .expect("scheduler event");
                for row in worker_log(claim, index).iter().chain([&scheduler]) {
                    (transaction, _) = append_recall(transaction, row).await.expect("append");
                }
            }
            commit(transaction).await.expect("commit");
            let nodes = "SELECT COUNT(*) AS count FROM sqb_run_nodes";
            let nodes_before = read(store, nodes).await;
            let scheduler = "SELECT COUNT(*) AS count FROM recall_events WHERE invocation_id LIKE 'scheduler/%'";
            let scheduler_before = read(store, scheduler).await;
            let retention = RunLogRetention::new(
                u16::try_from(case["keep_days"].as_u64().expect("days")).expect("u16"),
                u16::try_from(case["keep_runs"].as_u64().expect("runs")).expect("u16"),
            )
            .expect("retention");

            assert_eq!(
                prune_run_logs(store, retention).await.expect("prune") as u64,
                case["expected_pruned"].as_u64().expect("pruned"),
                "{backend:?}"
            );
            let logged: Vec<String> = case["expected_logged_runs"]
                .as_array()
                .expect("logged")
                .iter()
                .map(|index| {
                    let run_id = Uuid::from_u128(u128::from(index.as_u64().expect("index")));
                    format!("worker/{}", Uuid::new_v5(&run_id, b"run_claimed"))
                })
                .collect();
            let mut expected: Vec<Value> = logged
                .iter()
                .flat_map(|worker| [json!(worker), json!(format!("{worker}/session"))])
                .collect();
            expected.sort_by_key(ToString::to_string);
            let stored: Vec<Value> = read(
                store,
                "SELECT invocation_id FROM recall_output UNION SELECT invocation_id FROM recall_events \
                 WHERE invocation_id LIKE 'worker/%' UNION SELECT invocation_id FROM recall_artifact_refs \
                 ORDER BY invocation_id",
            )
            .await
            .into_iter()
            .map(|row| row["invocation_id"].clone())
            .collect();
            assert_eq!(
                stored, expected,
                "only the kept runs' logs remain, nested sessions included"
            );
            assert_eq!(
                read(store, "SELECT COUNT(*) AS count FROM recall_artifacts").await[0]["count"],
                json!(3),
                "the shared SQL and the kept runs' own SQL stay; the pruned runs' own SQL goes"
            );
            assert_eq!(
                read(store, scheduler).await,
                scheduler_before,
                "the scheduler's run history stays"
            );
            assert_eq!(
                read(store, "SELECT COUNT(*) AS count FROM sqb_runs").await[0]["count"],
                json!(4)
            );
            assert_eq!(read(store, nodes).await, nodes_before, "node results stay");
            assert_eq!(prune_run_logs(store, retention).await.expect("repeat"), 0);
            assert!(RunLogRetention::new(0, 1).is_err() && RunLogRetention::new(1, 0).is_err());
            close_test_store(fixture).await;
        });
    }
}
