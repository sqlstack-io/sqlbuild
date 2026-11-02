//! Actual ingestion and additive upgrade retain bounded, exact-destination build evidence.

use serde_json::{Value, json};
use uuid::Uuid;

use crate::credentials::models::{AuditContext, DefaultRole};
use crate::platform::main::report_location_health::report_location_health;
use crate::platform::models::{LocationHealth, LocationHealthReport};
use crate::store::errors::ControlError;
use crate::store::main::migrate_store::migrate_store;
use crate::store::models::ControlStore;
use crate::store::tests::store_helpers::reconstruct_fingerprint_predecessor;
use crate::warehouse_metadata::main::{
    ingest_fingerprints::ingest_fingerprints, read_person_fingerprint::read_person_fingerprint,
};
use crate::warehouse_metadata::models::{FingerprintBatch, FingerprintQuery};

pub(crate) async fn fingerprint_reads(
    store: &ControlStore,
    credentials: (&AuditContext, (&str, &str)),
    (location, other): (Uuid, Uuid),
) {
    let session_id = Uuid::now_v7();
    report_location_health(
        store,
        location,
        &LocationHealthReport {
            event_id: Uuid::now_v7(),
            session_id,
            target: "development".into(),
            health: LocationHealth::Heartbeat {},
        },
    )
    .await
    .expect("reader session");
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../benchmarks/duckshop/edge_cases/orchestration/fingerprint_lookup.json");
    let data: Value = serde_json::from_str(&std::fs::read_to_string(path).expect("fixture"))
        .expect("fingerprint inputs");
    let latest = data["record"].clone();
    let mut older = latest.clone();
    older["event_id"] = json!("orders-build-older");
    older["ts"] = data["older_ts"].clone();
    older["version_hash"] = json!("orders-version-older");
    let mut records = vec![latest.clone(), older];
    for (key, value) in [
        ("target_database", json!("orders_data")),
        ("target_schema", json!("landing")),
        ("target_name", json!("Orders")),
        ("node_type", json!("source")),
        ("node_name", json!("customers")),
    ] {
        let mut record = latest.clone();
        record["event_id"] = json!(format!("orders-distractor-{key}"));
        record["ts"] = data["distractor_ts"].clone();
        record[key] = value;
        records.push(record);
    }
    let batch: FingerprintBatch = serde_json::from_value(json!({
        "session_id":session_id,"generation":1,"records":records,
    }))
    .expect("batch");
    assert!(
        ingest_fingerprints(store, (location, "development"), &batch)
            .await
            .expect("ingest")
            .applied
    );
    let mut options = FingerprintQuery {
        node_type: "model".into(),
        node_name: "orders".into(),
        database: None,
        schema: Some("main".into()),
        name: Some("orders".into()),
    };
    let first = read_person_fingerprint(
        store,
        credentials,
        (location, "development", &options),
        DefaultRole::None,
    )
    .await
    .expect("latest build")
    .expect("evidence");
    let serialized = json!(&first);
    assert_eq!(
        serialized,
        json!({
            "event_id":first.event_id,"observed_at":latest["ts"],"source_event_id":latest["event_id"],
            "run_id":latest["run_id"],"definition_hash":latest["definition_hash"],
            "version_hash":latest["version_hash"],"schema_fingerprint":latest["schema_fingerprint"],
        })
    );
    for scope in [(other, "development"), (location, "preview")] {
        assert!(matches!(
            read_person_fingerprint(
                store,
                credentials,
                (scope.0, scope.1, &options),
                DefaultRole::None
            )
            .await,
            Err(ControlError::Denied { .. })
        ));
        assert!(
            read_person_fingerprint(
                store,
                credentials,
                (scope.0, scope.1, &options),
                DefaultRole::Viewer,
            )
            .await
            .expect("authorized empty scope")
            .is_none()
        );
    }
    options.schema = None;
    assert!(
        read_person_fingerprint(
            store,
            credentials,
            (location, "development", &options),
            DefaultRole::None
        )
        .await
        .expect("NULL schema is exact")
        .is_none()
    );
    options.schema = Some("main".into());
    options.name = None;
    assert!(
        read_person_fingerprint(
            store,
            credentials,
            (location, "development", &options),
            DefaultRole::None
        )
        .await
        .expect("NULL relation is exact")
        .is_none()
    );
    options.name = Some("orders".into());
    reconstruct_fingerprint_predecessor(store).await;
    migrate_store(store)
        .await
        .expect("populated fingerprint upgrade");
    migrate_store(store).await.expect("idempotent migration");
    assert!(
        !ingest_fingerprints(store, (location, "development"), &batch)
            .await
            .expect("replay")
            .applied
    );
    assert_eq!(
        json!(
            read_person_fingerprint(
                store,
                credentials,
                (location, "development", &options),
                DefaultRole::None
            )
            .await
            .expect("backfilled index")
        ),
        serialized
    );
}
