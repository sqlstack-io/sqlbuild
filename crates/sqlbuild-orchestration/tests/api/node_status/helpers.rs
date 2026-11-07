//! Real stored evidence never substitutes observation clocks for consumed inputs.

use serde_json::{Value, json};
use uuid::Uuid;

use crate::credentials::models::{AuditContext, DefaultRole};
use crate::runs::tests::helpers::snapshot;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::snapshots::models::SnapshotInput;
use crate::snapshots::models::SnapshotNode;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};
use crate::warehouse_metadata::main::{
    ingest_catalog::ingest_catalog, ingest_fingerprints::ingest_fingerprints,
    ingest_source_freshness::ingest_source_freshness,
    list_person_catalog_nodes::list_person_catalog_nodes,
    read_person_node_status::read_person_node_status,
};
use crate::warehouse_metadata::models::{MetadataBatch, NodeStatusQuery};

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
    publish_snapshot(store, &input)
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
    listing_reads_wide_targets_in_batches(store, credentials, (location, input.clone())).await;
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

/// Three thousand built nodes are read in batches, not one store round trip per node.
async fn listing_reads_wide_targets_in_batches(
    store: &ControlStore,
    credentials: (&AuditContext, Uuid),
    (location, mut input): (Uuid, SnapshotInput),
) {
    let names: Vec<String> = (0..3_000).map(|index| format!("wide_{index:04}")).collect();
    for name in &names {
        input.nodes.insert(format!("model:{name}"), SnapshotNode {
            kind: "model".into(),
            definition: json!({"name":name,"destination":{"database":null,"schema":"main","name":name}}),
            raw_sql: None,
            compiled_sql: None,
            python_source: None,
        });
    }
    input.generation += 1;
    publish_snapshot(store, &input)
        .await
        .expect("wide snapshot");
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
    for chunk in names.chunks(300) {
        let mut parameters = vec![Parameter::Id(template)];
        let mut values: Vec<String> = Vec::new();
        for name in chunk {
            values.push(format!(
                "(${},${},${})",
                parameters.len() + 1,
                parameters.len() + 2,
                parameters.len() + 3
            ));
            parameters.push(Parameter::Id(Uuid::now_v7()));
            parameters.push(Parameter::Text(format!("wide-{name}")));
            parameters.push(Parameter::Text(name.clone()));
        }
        (transaction, _) = query(
            transaction,
            &format!(
                "INSERT INTO sqb_warehouse_fingerprints (event_id,source_event_id,node_name,target_name,location_id,target, \
                   observed_at,definition_text_hash,record_json,received_at,state_database,state_schema,node_type, \
                   target_database,target_schema) \
                 SELECT wide.column1,wide.column2,wide.column3,wide.column3,location_id,target,observed_at, \
                   definition_text_hash,record_json,received_at,state_database,state_schema,node_type, \
                   target_database,target_schema \
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
        let built = listed
            .iter()
            .filter(|node| node.node_key.starts_with("model:wide_"))
            .filter(|node| {
                node.last_build
                    .as_ref()
                    .is_some_and(|build| json!(build.run_id) == rows[0]["run_id"])
            })
            .count();
        assert_eq!(built, names.len(), "every wide node keeps its own build");
    }
    timings.sort();
    assert!(
        timings[1] < std::time::Duration::from_millis(450),
        "listing {} nodes took {:?}",
        names.len(),
        timings[1]
    );
}
