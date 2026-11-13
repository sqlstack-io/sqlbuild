//! Run fixtures exercise native queues and Recall against isolated real databases.

use std::collections::{BTreeMap, BTreeSet};

use crate::api::errors::ApiError;
use crate::authorization::main::assign_builtin_role::assign_builtin_role;
use crate::authorization::main::bootstrap_user::bootstrap_user;
use crate::credentials::main::identities::create_principal::create_principal;
use crate::credentials::models::{AuditContext, BuiltinRole, RoleAssignmentRequest};
use crate::credentials::tests::location_helpers::create_location;
use crate::runs::_helpers::requests::publication::publish_request;
use crate::runs::models::{
    AdhocCheck, AdhocCheckRequest, AdhocRunRequest, RunCommand, RunKind, RunOrigin, RunRequest,
};
use crate::runs::tests::test_types::{AdhocCommandTestCase, AdhocPeople};
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::snapshots::models::{SnapshotInput, SnapshotNode};
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};
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

use crate::store::main::append_recall::append_recall;

pub(crate) async fn request_run(
    store: &ControlStore,
    request: &RunRequest,
) -> Result<bool, ControlError> {
    let transaction = begin(store, TransactionMode::Write).await?;
    let (transaction, applied) = publish_request(transaction, request).await?;
    commit(transaction).await?;
    Ok(applied)
}

pub(crate) fn snapshot(location_id: Uuid) -> SnapshotInput {
    SnapshotInput {
        location_id,
        target: "development".into(),
        generation: 1,
        git_revision: None,
        git_dirty: false,
        git_author: None,
        image_id: Some("sha256:synthetic-orders-image".into()),
        environment_hash: "orders-environment".into(),
        sqb_version: "1.0.0".into(),
        nodes: ["customers", "inventory", "orders", "products"]
            .into_iter()
            .map(|name| {
                (
                    name.into(),
                    SnapshotNode {
                        kind: "model".into(),
                        definition: json!({"materialization": "table"}),
                        raw_sql: Some("select 1 as id".into()),
                        compiled_sql: Some("select 1 as id".into()),
                        python_source: None,
                    },
                )
            })
            .collect(),
        documents: BTreeMap::new(),
    }
}

pub(crate) fn request(
    (location_id, snapshot_id): (Uuid, Uuid),
    id: u128,
    priority: i64,
    nodes: &[&str],
) -> RunRequest {
    let selection: BTreeSet<String> = nodes.iter().map(|name| (*name).into()).collect();
    let run_id = Uuid::from_u128(id);
    RunRequest {
        run_id,
        location_id,
        target: "development".into(),
        snapshot_id,
        image_id: Some("sha256:synthetic-orders-image".into()),
        kind: RunKind::Adhoc,
        command: RunCommand::Build,
        origin: RunOrigin::Manual,
        job_key: None,
        parent_run_id: None,
        root_run_id: run_id,
        trigger_id: None,
        tick_id: None,
        rerun_of_run_id: None,
        requested_by: "test-operator".into(),
        priority,
        write_set: selection.clone(),
        selection,
        settings: BTreeMap::new(),
        window: None,
        concurrency_groups: BTreeSet::new(),
        narrowing: None,
    }
}

pub(crate) async fn queue(store: &ControlStore, request: &RunRequest) {
    assert!(
        request_run(store, request)
            .await
            .expect("queue pinned request")
    );
    assert!(
        !request_run(store, request)
            .await
            .expect("retry pinned request")
    );
    let mut changed = request.clone();
    changed.priority += 1;
    assert!(
        request_run(store, &changed).await.is_err(),
        "a reused run ID cannot change its request"
    );
}

/// Rows of `sql` as JSON, read in their own transaction.
pub(crate) async fn rows(
    store: &ControlStore,
    sql: &str,
    parameters: Vec<Parameter>,
) -> Vec<Value> {
    let transaction = begin(store, TransactionMode::Read).await.expect("read");
    let (transaction, rows) = query(transaction, sql, parameters).await.expect("query");
    commit(transaction).await.expect("commit");
    rows.into_iter().map(|row| json!(row)).collect()
}

/// A JSON column, which SQLite returns as text and PostgreSQL as JSON.
pub(crate) fn json_column(value: &Value) -> Value {
    value.as_str().map_or(value.clone(), |text| {
        serde_json::from_str(text).expect("JSON column")
    })
}

/// Two models, a seed, a read-only source, a customers source loaded after accounts and one person per role under test.
pub(crate) async fn adhoc_fixture(
    store: &ControlStore,
    context: &AuditContext,
) -> (Uuid, Uuid, AdhocPeople) {
    let location_id = create_location(store, "orders", "test-owner")
        .await
        .expect("location");
    let mut input = snapshot(location_id);
    input.nodes = [
        "model:orders",
        "model:customers",
        "seed:currencies",
        "source:shop.orders",
    ]
    .into_iter()
    .map(published_node)
    .collect();
    let (key, mut managed) = published_node("source:shop.customers");
    managed.definition =
        json!({"entry": {"loader": "load_customers"}, "loads": ["source:shop.accounts"]});
    input.nodes.insert(key, managed);
    let (key, mut upstream) = published_node("source:shop.accounts");
    upstream.definition = json!({"entry": {"loader": "load_accounts"}});
    input.nodes.insert(key, upstream);
    input
        .nodes
        .get_mut("model:customers")
        .expect("customers model")
        .definition = json!({"materialization": "table", "deps": ["source:shop.customers"]});
    let published = publish_snapshot(store, &input).await.expect("publish");
    let admin = bootstrap_user(store, context, "orders-admin")
        .await
        .expect("administrator");
    let developer = person_with(
        store,
        (context, admin),
        "orders-developer",
        &[BuiltinRole::Developer],
    )
    .await;
    let operator = person_with(
        store,
        (context, admin),
        "orders-operator",
        &[BuiltinRole::Operator],
    )
    .await;
    let stranger = person_with(store, (context, admin), "orders-stranger", &[]).await;
    (
        location_id,
        published.snapshot_id,
        AdhocPeople {
            admin,
            developer,
            operator,
            stranger,
        },
    )
}

fn published_node(key: &str) -> (String, SnapshotNode) {
    let kind = key.split_once(':').expect("kind").0;
    (
        key.to_owned(),
        SnapshotNode {
            kind: kind.into(),
            definition: json!({"materialization": "table"}),
            raw_sql: None,
            compiled_sql: Some("select 1 as id".into()),
            python_source: None,
        },
    )
}

async fn person_with(
    store: &ControlStore,
    (context, admin): (&AuditContext, Uuid),
    name: &str,
    roles: &[BuiltinRole],
) -> Uuid {
    let person = create_principal(store, context, name)
        .await
        .expect("person");
    for role in roles.iter().copied() {
        assign_builtin_role(
            store,
            (context, admin),
            &RoleAssignmentRequest {
                principal_id: person,
                role: role.into(),
                location_id: None,
                target: None,
                revoke: false,
            },
        )
        .await
        .expect("assign role");
    }
    person
}

/// The case's command as a dry run in the fixture's location.
pub(crate) fn adhoc_check(
    location_id: Uuid,
    test_case: &AdhocCommandTestCase,
) -> AdhocCheckRequest {
    AdhocCheckRequest {
        location_id,
        target: "development".into(),
        command: test_case
            .command
            .iter()
            .map(|token| (*token).to_owned())
            .collect(),
        priority: test_case.priority,
    }
}

/// The case's command as a launch with its own run ID.
pub(crate) fn adhoc_launch(
    location_id: Uuid,
    test_case: &AdhocCommandTestCase,
    run_id: Uuid,
) -> AdhocRunRequest {
    let check = adhoc_check(location_id, test_case);
    AdhocRunRequest {
        run_id,
        location_id,
        target: check.target,
        command: check.command,
        reason: test_case.reason.into(),
        priority: test_case.priority,
    }
}

/// What a dry run decided, in the shape the cases name it.
pub(crate) fn check_projection(check: &AdhocCheck) -> Value {
    json!({
        "selection": check.selection,
        "write_set": check.write_set,
        "permissions": check.permissions.iter().map(|permission| json!([permission.action, permission.allowed])).collect::<Vec<Value>>(),
        "error": check.error,
        "launchable": check.launchable,
    })
}

/// Whether a launch applied, or the HTTP status its refusal reaches people with.
pub(crate) fn launch_outcome(result: Result<bool, ControlError>) -> Result<bool, u16> {
    result.map_err(|error| ApiError::from(error).status.as_u16())
}

/// One SQL text every logged run shares.
const SHARED_LOG_SQL: &str = "select order_id from orders";

fn log_envelope(invocation_id: String) -> RowEnvelope {
    RowEnvelope {
        invocation_id,
        invocation_started_at: "2026-01-02T03:04:05Z".into(),
        producer_version: "test".into(),
    }
}

fn log_sql_rows(envelope: &RowEnvelope, sql: &str) -> [RecallRow; 2] {
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
    let worker = log_envelope(format!("worker/{claim}"));
    let session = log_envelope(format!("worker/{claim}/session"));
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
    rows.extend(log_sql_rows(&session, SHARED_LOG_SQL));
    rows.extend(log_sql_rows(
        &session,
        &format!("select {index} as order_id"),
    ));
    rows
}

/// The worker invocation of run `index`, claimed by the claim ID `seed_logged_runs` gives it.
pub(crate) fn logged_worker(index: u128) -> String {
    format!(
        "worker/{}",
        Uuid::new_v5(&Uuid::from_u128(index), b"run_claimed")
    )
}

/// Ad-hoc runs 1, 2, ... of `location`, each ended the given hours ago or, for null, still running, with a worker log and a scheduler event.
pub(crate) async fn seed_logged_runs(
    store: &ControlStore,
    (location, snapshot_id): (Uuid, Uuid),
    ended_hours_ago: &[Value],
) {
    for (position, hours) in ended_hours_ago.iter().enumerate() {
        let index = position as u128 + 1;
        let run_request = request((location, snapshot_id), index, 0, &["orders"]);
        let run_id = run_request.run_id;
        assert!(request_run(store, &run_request).await.expect("request"));
        let transaction = begin(store, TransactionMode::Write).await.expect("begin");
        let claim = Uuid::new_v5(&run_id, b"run_claimed");
        let ended_at = hours
            .as_i64()
            .map(|hours| transaction.now - TimeDelta::hours(hours));
        let status = ended_at.map_or("running", |_| "success");
        let (mut transaction, _) = query(
            transaction,
            "UPDATE sqb_runs SET status=$1, worker_instance=$2, ended_at=$3 WHERE run_id=$4",
            vec![
                Parameter::Text(status.into()),
                Parameter::Id(claim),
                Parameter::OptionalTime(ended_at),
                Parameter::Id(run_id),
            ],
        )
        .await
        .expect("finish run");
        let scheduler = event_object_row(
            &log_envelope(format!("scheduler/{run_id}")),
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
        commit(transaction).await.expect("commit");
    }
}
