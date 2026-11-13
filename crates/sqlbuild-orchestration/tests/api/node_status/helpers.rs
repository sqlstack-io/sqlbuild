//! Real stored evidence never substitutes observation clocks for consumed inputs.

use serde_json::{Value, json};
use std::cell::Cell;
use uuid::Uuid;

use crate::credentials::models::{AuditContext, DefaultRole};
use crate::runs::tests::helpers::{request, request_run, snapshot};
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::snapshots::models::SnapshotInput;
use crate::snapshots::models::SnapshotNode;
use crate::store::_helpers::statement_count::STATEMENTS;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};
use crate::warehouse_metadata::main::{
    ingest::ingest_catalog::ingest_catalog, ingest::ingest_fingerprints::ingest_fingerprints,
    ingest::ingest_source_freshness::ingest_source_freshness,
    person_reads::list_person_catalog_nodes::list_person_catalog_nodes,
    person_reads::read_person_node_status::read_person_node_status,
};
use crate::warehouse_metadata::models::{MetadataBatch, NodeStatusQuery};

/// Statements a catalog listing issues besides one build read per 100 nodes: 8 fixed, 1 per extra namespace shape.
const WIDE_LISTING_FIXED_STATEMENTS: usize = 9;

pub(crate) async fn node_status_reads(
    store: &ControlStore,
    credentials: (&AuditContext, Uuid),
    (location, session_id): (Uuid, Uuid),
) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../benchmarks/duckshop/edge_cases/orchestration");
    let data: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("node_status.json")).expect("fixture"),
    )
    .expect("status inputs");
    let mut input = snapshot(location);
    input.generation = 2;
    input.nodes.insert("model:orders".into(), SnapshotNode {
        kind: "model".into(),
        definition: json!({"name":"orders","destination":{"database":null,"schema":"main","name":"orders"},
            "planner_identity":{"local_hash":data["model_local_hash"]},"deps":["source:raw_orders"]}),
        raw_sql: None, compiled_sql: None, python_source: None,
    });
    input.nodes.insert(
        "udf:orders".into(),
        SnapshotNode {
            kind: "udf".into(),
            definition: data["same_named_function"].clone(),
            raw_sql: None,
            compiled_sql: Some("23".into()),
            python_source: None,
        },
    );
    let published = publish_snapshot(store, &input)
        .await
        .expect("identity snapshot");
    let options = NodeStatusQuery {
        node_key: "model:orders".into(),
    };
    let initial = json!(
        read_person_node_status(
            store,
            credentials,
            (location, "development", &options),
            DefaultRole::None
        )
        .await
        .expect("legacy build")
    );
    assert_eq!(initial["definition"]["status"], "unavailable");
    assert_eq!(initial["physical"]["status"], "unavailable");
    let fixture: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("fingerprint_lookup.json")).expect("build fixture"),
    )
    .expect("build inputs");
    let mut record = fixture["record"].clone();
    let mut metadata = data["model_metadata"].clone();
    metadata["audit_gate"] = json!({"run_id":"audit-run","outcome":"passed"});
    metadata["migration_fingerprint"] = json!("migration-v1");
    metadata["declared_columns_hash"] = json!("columns-v1");
    record["metadata_json"] = json!(metadata.to_string());
    record["event_id"] = json!("orders-model-identity");
    record["ts"] = json!("2026-01-05T10:00:00Z");
    let batch: MetadataBatch = serde_json::from_value(
        json!({"kind":"fingerprints","session_id":session_id,"generation":2,"records":[record]}),
    )
    .expect("model build");
    ingest_fingerprints(store, (location, "development"), &batch)
        .await
        .expect("build ingestion");
    let before = json!(
        read_person_node_status(
            store,
            credentials,
            (location, "development", &options),
            DefaultRole::None
        )
        .await
        .expect("matched definition without catalog")
    );
    assert_eq!(
        before["definition"],
        json!({"status":"matched","local_hash":data["model_local_hash"]})
    );
    assert_eq!(before["physical"]["status"], "unavailable");
    let catalog: MetadataBatch = serde_json::from_value(json!({"kind":"catalog","session_id":session_id,"generation":2,"records":[data["physical"]],
        "scan":{"scan_id":Uuid::now_v7(),"location":{"database":null,"schema":"main"},"complete":false}})).expect("catalog page");
    ingest_catalog(store, (location, "development"), &catalog)
        .await
        .expect("physical evidence");
    let freshness: MetadataBatch = serde_json::from_value(
        json!({"kind":"source_freshness","session_id":session_id,"generation":2,"records":[data["freshness"]]}),
    )
    .expect("newer source observation");
    assert!(
        serde_json::from_value::<chrono::DateTime<chrono::Utc>>(
            freshness.records[0]["observed_at"].clone()
        )
        .expect("observed_at")
            > serde_json::from_value::<chrono::DateTime<chrono::Utc>>(
                batch.records[0]["ts"].clone()
            )
            .expect("ts")
    );
    ingest_source_freshness(store, (location, "development"), &freshness)
        .await
        .expect("source observation");
    let after = json!(
        read_person_node_status(
            store,
            credentials,
            (location, "development", &options),
            DefaultRole::None
        )
        .await
        .expect("independent evidence")
    );
    assert_eq!(after["physical"]["status"], "observed");
    assert_eq!(after["physical"]["observation"]["record"], data["physical"]);
    assert_eq!(after["definition"], before["definition"]);
    assert_eq!(after["source_data"], before["source_data"]);
    assert_eq!(
        after["source_data"],
        json!({"status":"unknown","reason":"consumed-input provenance is not recorded"})
    );
    let mut function_batch = batch.clone();
    function_batch.records[0]["node_type"] = json!("udf");
    function_batch.records[0]["event_id"] = json!("orders-function-build");
    function_batch.records[0]["run_id"] = json!("orders-function-run");
    ingest_fingerprints(store, (location, "development"), &function_batch)
        .await
        .expect("same-named function build");
    let function_status = json!(
        read_person_node_status(
            store,
            credentials,
            (
                location,
                "development",
                &NodeStatusQuery {
                    node_key: "udf:orders".into()
                }
            ),
            DefaultRole::None
        )
        .await
        .expect("function namespace")
    );
    assert_eq!(function_status["physical"]["status"], "unavailable");
    assert_eq!(function_status["definition"]["status"], "unavailable");
    assert_eq!(function_status["build"]["run_id"], "orders-function-run");
    assert_eq!(function_status["source_data"], after["source_data"]);
    let mut changed = batch.clone();
    metadata["config"]["lookback"] = json!("7d");
    changed.records[0]["metadata_json"] = json!(metadata.to_string());
    changed.records[0]["event_id"] = json!("orders-config-changed");
    changed.records[0]["ts"] = json!(
        serde_json::from_value::<chrono::DateTime<chrono::Utc>>(changed.records[0]["ts"].clone())
            .expect("ts")
            + chrono::Duration::days(1)
    );
    ingest_fingerprints(store, (location, "development"), &changed)
        .await
        .expect("config-only build");
    let changed_status = json!(
        read_person_node_status(
            store,
            credentials,
            (location, "development", &options),
            DefaultRole::None
        )
        .await
        .expect("config comparison")
    );
    assert_eq!(changed_status["definition"]["status"], "changed");
    assert_eq!(
        changed_status["definition"]["current_local_hash"],
        data["model_local_hash"]
    );
    assert_ne!(
        changed_status["definition"]["recorded_local_hash"],
        data["model_local_hash"]
    );
    assert_eq!(changed_status["source_data"], after["source_data"]);
    assert!(!changed_status.to_string().contains("metadata_json"));
    let listed: Vec<Value> = list_person_catalog_nodes(
        store,
        credentials,
        (location, "development"),
        DefaultRole::None,
    )
    .await
    .expect("catalog nodes")
    .into_iter()
    .map(|node| json!(node))
    .collect();
    let listed_node = |key: &str| {
        listed
            .iter()
            .find(|node| node["node_key"] == key)
            .cloned()
            .unwrap_or_else(|| panic!("{key} listed"))
    };
    let orders = listed_node("model:orders");
    assert_eq!(
        orders["last_build"]["observed_at"],
        json!(changed.records[0]["ts"]),
        "the newest build of the model wins"
    );
    assert_eq!(orders["last_build"]["run_id"], batch.records[0]["run_id"]);
    assert_eq!(
        orders["relation"],
        json!({"database": null, "schema": "main", "name": "orders"})
    );
    assert_eq!(orders["dependencies"], json!(["source:raw_orders"]));
    assert_eq!(
        orders["definition_changed"], true,
        "the listing compares definitions like the node status"
    );
    assert_eq!(
        orders["last_outcome"],
        json!({"status": "success", "at": changed.records[0]["ts"], "run_id": null}),
        "a build outside the scheduler is a success"
    );
    assert_eq!(
        listed_node("udf:orders")["last_build"]["run_id"],
        "orders-function-run",
        "a same-named function keeps its own build"
    );
    assert!(
        listed
            .iter()
            .filter(|node| node["relation"].is_null())
            .all(|node| node["last_build"].is_null())
    );
    assert!(!json!(listed).to_string().contains("metadata_json"));
    listing_ignores_build_history(store, credentials, (location, &listed)).await;
    listing_reports_scheduler_attempts(
        store,
        credentials,
        (location, published.snapshot_id),
        serde_json::from_value(changed.records[0]["ts"].clone()).expect("build time"),
    )
    .await;
    listing_wide_targets_is_batched_without_super_linear_growth(
        store,
        credentials,
        (location, input.clone()),
    )
    .await;
    assert!(matches!(
        read_person_node_status(
            store,
            credentials,
            (location, "preview", &options),
            DefaultRole::None
        )
        .await,
        Err(ControlError::Denied { .. })
    ));
    assert!(
        read_person_node_status(
            store,
            credentials,
            (
                location,
                "development",
                &NodeStatusQuery {
                    node_key: "model:unlisted".into()
                }
            ),
            DefaultRole::None
        )
        .await
        .expect("unknown node")
        .is_none()
    );
}

/// Older builds copied 200,000 times over must not change the listing or make it scan them.
async fn listing_ignores_build_history(
    store: &ControlStore,
    credentials: (&AuditContext, Uuid),
    (location, listed): (Uuid, &[Value]),
) {
    let transaction = begin(store, TransactionMode::Write).await.expect("begin");
    let (mut transaction, rows) = query(
        transaction,
        "SELECT event_id FROM sqb_warehouse_fingerprints WHERE location_id=$1 AND target='development'",
        vec![Parameter::Id(location)],
    )
    .await
    .expect("current builds");
    let templates: Vec<Uuid> = rows
        .iter()
        .map(|row| serde_json::from_value(row["event_id"].clone()).expect("event id"))
        .collect();
    let copies = 200_000 / templates.len();
    let oldest = chrono::DateTime::parse_from_rfc3339("2025-01-01T00:00:00Z")
        .expect("history start")
        .to_utc();
    for template in &templates {
        for first in (0..copies).step_by(1_000) {
            let mut parameters = vec![Parameter::Id(*template)];
            let mut values: Vec<String> = Vec::new();
            for copy in first..copies.min(first + 1_000) {
                values.push(format!(
                    "(${},${},${})",
                    parameters.len() + 1,
                    parameters.len() + 2,
                    parameters.len() + 3
                ));
                parameters.push(Parameter::Id(Uuid::now_v7()));
                parameters.push(Parameter::Text(format!("history-{template}-{copy}")));
                parameters.push(Parameter::Time(
                    oldest + chrono::Duration::seconds(i64::try_from(copy).expect("seconds")),
                ));
            }
            (transaction, _) = query(
                transaction,
                &format!(
                    "INSERT INTO sqb_warehouse_fingerprints (event_id,source_event_id,observed_at,location_id,target, \
                       definition_text_hash,record_json,received_at,state_database,state_schema,node_type,node_name, \
                       target_database,target_schema,target_name) \
                     SELECT history.column1,history.column2,history.column3,location_id,target,definition_text_hash, \
                       record_json,received_at,state_database,state_schema,node_type,node_name,target_database, \
                       target_schema,target_name \
                     FROM (VALUES {}) AS history, sqb_warehouse_fingerprints WHERE event_id=$1",
                    values.join(",")
                ),
                parameters,
            )
            .await
            .expect("older build history");
        }
    }
    commit(transaction).await.expect("commit history");
    let mut timings: Vec<std::time::Duration> = Vec::new();
    for _ in 0..3 {
        let started = std::time::Instant::now();
        let relisted: Vec<Value> = list_person_catalog_nodes(
            store,
            credentials,
            (location, "development"),
            DefaultRole::None,
        )
        .await
        .expect("catalog nodes over a long history")
        .into_iter()
        .map(|node| json!(node))
        .collect();
        timings.push(started.elapsed());
        assert_eq!(relisted, listed, "older builds never replace the newest");
    }
    timings.sort();
    assert!(
        timings[1] < std::time::Duration::from_millis(100),
        "listing {} nodes over {} builds took {:?}",
        listed.len(),
        templates.len() * (copies + 1),
        timings[1]
    );
}

/// A failed scheduler attempt replaces the outcome only when it ended after the newest build, and stays after quicker runs.
async fn listing_reports_scheduler_attempts(
    store: &ControlStore,
    credentials: (&AuditContext, Uuid),
    (location, snapshot_id): (Uuid, Uuid),
    built_at: chrono::DateTime<chrono::Utc>,
) {
    let outcome = || async {
        let listed = list_person_catalog_nodes(
            store,
            credentials,
            (location, "development"),
            DefaultRole::None,
        )
        .await
        .expect("catalog nodes");
        let orders = listed
            .into_iter()
            .find(|node| node.node_key == "model:orders")
            .expect("orders listed");
        json!(orders.last_outcome)
    };
    for (id, ended_at, expected) in [
        (
            0x0f01,
            built_at - chrono::Duration::hours(1),
            json!({"status": "success", "at": built_at, "run_id": null}),
        ),
        (
            0x0f02,
            built_at + chrono::Duration::hours(1),
            json!({"status": "failed", "at": built_at + chrono::Duration::hours(1),
                "run_id": Uuid::from_u128(0x0f02)}),
        ),
    ] {
        let run = request((location, snapshot_id), id, 0, &["model:orders"]);
        request_run(store, &run).await.expect("queue attempt");
        let transaction = begin(store, TransactionMode::Write).await.expect("begin");
        let (transaction, _) = query(
            transaction,
            "UPDATE sqb_run_nodes SET status='failed',started_at=$1,ended_at=$2,finished_storage_id=$3 WHERE run_id=$4",
            vec![
                Parameter::Time(ended_at - chrono::Duration::minutes(5)),
                Parameter::Time(ended_at),
                Parameter::Integer(i64::try_from(id).expect("storage id")),
                Parameter::Id(run.run_id),
            ],
        )
        .await
        .expect("finish attempt");
        let (transaction, _) = query(
            transaction,
            "UPDATE sqb_runs SET status='failed' WHERE run_id=$1",
            vec![Parameter::Id(run.run_id)],
        )
        .await
        .expect("fail run");
        commit(transaction).await.expect("commit attempt");
        assert_eq!(outcome().await, expected, "attempt ending at {ended_at}");
    }
    for id in 0x0f10..0x0f10 + 25 {
        request_run(
            store,
            &request((location, snapshot_id), id, 0, &["customers"]),
        )
        .await
        .expect("newer run of another node");
    }
    assert_eq!(
        outcome().await,
        json!({"status": "failed", "at": built_at + chrono::Duration::hours(1),
            "run_id": Uuid::from_u128(0x0f02)}),
        "a failure stays reported after more than 25 newer runs"
    );
}

/// Three thousand nodes, half in a named database, keep their own builds, read 100 per statement without super-linear growth.
async fn listing_wide_targets_is_batched_without_super_linear_growth(
    store: &ControlStore,
    credentials: (&AuditContext, Uuid),
    (location, mut input): (Uuid, SnapshotInput),
) {
    let names: Vec<String> = (0..3_000).map(|index| format!("wide_{index:04}")).collect();
    let mut medians: Vec<std::time::Duration> = Vec::new();
    for (first, size) in [(0, 1_000), (1_000, names.len())] {
        let (median, statements) = list_wide_target(
            store,
            credentials,
            (location, &mut input),
            (&names, first, size),
        )
        .await;
        assert!(
            statements <= size.div_ceil(100) + WIDE_LISTING_FIXED_STATEMENTS,
            "listing {size} nodes issued {statements} store statements; builds must be read 100 or more nodes per statement"
        );
        medians.push(median);
    }
    let ratio: f64 = medians[1].as_secs_f64() / medians[0].as_secs_f64();
    assert!(
        ratio < 6.0,
        "listing {} nodes took {:?}, {ratio:.1} times the {:?} for 1,000; listing must not grow super-linearly",
        names.len(),
        medians[1],
        medians[0]
    );
}

/// Add `names[first..size]` with one build each; return the median listing time and one listing's statement count.
async fn list_wide_target(
    store: &ControlStore,
    credentials: (&AuditContext, Uuid),
    (location, input): (Uuid, &mut SnapshotInput),
    (names, first, size): (&[String], usize, usize),
) -> (std::time::Duration, usize) {
    let database = |index: usize| (index % 2 == 1).then(|| "analytics_db".to_owned());
    let built_at = |index: usize| {
        chrono::DateTime::parse_from_rfc3339("2026-02-01T00:00:00Z")
            .expect("build start")
            .to_utc()
            + chrono::Duration::seconds(i64::try_from(index).expect("seconds"))
    };
    for (index, name) in names.iter().enumerate().take(size).skip(first) {
        input.nodes.insert(format!("model:{name}"), SnapshotNode {
            kind: "model".into(),
            definition: json!({"name":name,"destination":{"database":database(index),"schema":"main","name":name}}),
            raw_sql: None,
            compiled_sql: None,
            python_source: None,
        });
    }
    input.generation += 1;
    publish_snapshot(store, input).await.expect("wide snapshot");
    let transaction = begin(store, TransactionMode::Write).await.expect("begin");
    let (mut transaction, rows) = query(
        transaction,
        "SELECT event_id,record_json ->> 'run_id' AS run_id FROM sqb_warehouse_fingerprints \
         WHERE location_id=$1 AND target='development' AND node_type='model' AND node_name='orders' \
         AND target_database IS NULL AND target_schema='main' AND target_name='orders' LIMIT 1",
        vec![Parameter::Id(location)],
    )
    .await
    .expect("template build");
    let template: Uuid = serde_json::from_value(rows[0]["event_id"].clone()).expect("event id");
    for (chunk_index, chunk) in names[first..size].chunks(300).enumerate() {
        let mut parameters = vec![Parameter::Id(template)];
        let mut values: Vec<String> = Vec::new();
        for (offset, name) in chunk.iter().enumerate() {
            let index = first + chunk_index * 300 + offset;
            values.push(format!(
                "(${},${},${},${},${})",
                parameters.len() + 1,
                parameters.len() + 2,
                parameters.len() + 3,
                parameters.len() + 4,
                parameters.len() + 5
            ));
            parameters.push(Parameter::Id(Uuid::now_v7()));
            parameters.push(Parameter::Text(format!("wide-{name}")));
            parameters.push(Parameter::Text(name.clone()));
            parameters.push(Parameter::Time(built_at(index)));
            parameters.push(Parameter::OptionalText(database(index)));
        }
        (transaction, _) = query(
            transaction,
            &format!(
                "INSERT INTO sqb_warehouse_fingerprints (event_id,source_event_id,node_name,target_name,location_id,target, \
                   observed_at,target_database,definition_text_hash,record_json,received_at,state_database,state_schema, \
                   node_type,target_schema) \
                 SELECT wide.column1,wide.column2,wide.column3,wide.column3,location_id,target,wide.column4,wide.column5, \
                   definition_text_hash,record_json,received_at,state_database,state_schema,node_type,target_schema \
                 FROM (VALUES {}) AS wide, sqb_warehouse_fingerprints WHERE event_id=$1",
                values.join(",")
            ),
            parameters,
        )
        .await
        .expect("wide builds");
    }
    commit(transaction).await.expect("commit builds");
    let mut timings: Vec<std::time::Duration> = Vec::new();
    for _ in 0..3 {
        let started = std::time::Instant::now();
        let listed = list_person_catalog_nodes(
            store,
            credentials,
            (location, "development"),
            DefaultRole::None,
        )
        .await
        .expect("wide catalog");
        timings.push(started.elapsed());
        let builds: std::collections::BTreeMap<&str, _> = listed
            .iter()
            .map(|node| (node.node_key.as_str(), node.last_build.as_ref()))
            .collect();
        let built = names[..size]
            .iter()
            .enumerate()
            .filter(|(index, name)| {
                builds
                    .get(format!("model:{name}").as_str())
                    .copied()
                    .flatten()
                    .is_some_and(|build| {
                        build.observed_at == built_at(*index)
                            && json!(build.run_id) == rows[0]["run_id"]
                    })
            })
            .count();
        assert_eq!(built, size, "every wide node keeps its own build");
    }
    timings.sort();
    let statements = STATEMENTS
        .scope(Cell::new(0), async {
            list_person_catalog_nodes(
                store,
                credentials,
                (location, "development"),
                DefaultRole::None,
            )
            .await
            .expect("counted wide catalog");
            STATEMENTS.with(Cell::get)
        })
        .await;
    (timings[1], statements)
}
