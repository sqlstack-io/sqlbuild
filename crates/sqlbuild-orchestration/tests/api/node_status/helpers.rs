//! Real stored evidence never substitutes observation clocks for consumed inputs.

use serde_json::{Value, json};
use uuid::Uuid;

use crate::credentials::models::{AuditContext, DefaultRole};
use crate::runs::tests::helpers::snapshot;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::snapshots::models::SnapshotNode;
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;
use crate::warehouse_metadata::main::{
    ingest_catalog::ingest_catalog, ingest_fingerprints::ingest_fingerprints,
    ingest_source_freshness::ingest_source_freshness,
    read_person_node_status::read_person_node_status,
};
use crate::warehouse_metadata::models::{MetadataBatch, NodeStatusQuery};

pub(crate) async fn node_status_reads(
    store: &ControlStore,
    credentials: (&AuditContext, (&str, &str)),
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
