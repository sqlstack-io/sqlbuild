//! Exact definitions survive semantic-hash reuse, page retries and mirror rebuilds.

use reqwest::{Client, StatusCode};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::platform::models::LocationWorkClient;
use crate::store::main::decompress::decompress;
use crate::store::main::{
    begin::begin, commit::commit, migrate_store::migrate_store, query::query,
};
use crate::store::models::{ControlStore, TransactionMode};
use crate::store::tests::fault_helpers::{install_fault, remove_fault};
use crate::store::tests::fault_types::TransactionFault;
use crate::warehouse_metadata::models::FingerprintBatch;

pub(crate) async fn fingerprint_history(
    store: &ControlStore,
    outbound: &LocationWorkClient,
    (session_id, preview_session_id): (Uuid, Uuid),
    (client, freshness_url, bearer): (&Client, &str, &str),
) {
    // Upgrade the immediate predecessor while published snapshots and freshness facts exist.
    let mut transaction = begin(store, TransactionMode::Write).await.expect("upgrade");
    for table in [
        "sqb_external_sessions",
        "sqb_role_assignments",
        "sqb_admin_initialization",
        "sqb_oidc_login_flows",
        "sqb_authentication_revision",
        "sqb_password_sessions",
        "sqb_password_login_attempts",
    ] {
        (transaction, _) = query(transaction, &format!("DROP TABLE {table}"), vec![])
            .await
            .expect("remove subsequent session migration");
    }
    let (transaction, _) = query(transaction, "DROP TABLE sqb_password_credentials", vec![])
        .await
        .expect("remove subsequent password migration");
    let (transaction, _) = query(transaction, "DROP INDEX sqb_node_decisive_history", vec![])
        .await
        .expect("remove subsequent transition migration");
    let (transaction, _) = query(transaction, "DROP TABLE sqb_development_locations", vec![])
        .await
        .expect("remove subsequent development migration");
    let (transaction, _) = query(transaction, "DROP TABLE sqb_warehouse_fingerprints", vec![])
        .await
        .expect("predecessor table set");
    let (transaction, _) = query(
        transaction,
        "DELETE FROM sqb_schema_migrations WHERE name IN ('warehouse_fingerprints','development_locations','node_transitions','password_credentials','password_sessions','authentication_revision','oidc_login_flows','administrator_initialization','external_sessions')",
        vec![],
    )
    .await
    .expect("predecessor schema version");
    commit(transaction).await.expect("populated predecessor");
    migrate_store(store).await.expect("additive mirror upgrade");
    let url = freshness_url.replace("source-freshness", "fingerprints");
    let variants_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../benchmarks/duckshop/edge_cases/orchestration/fingerprint_definition_variants.json",
    );
    let variants: Value =
        serde_json::from_str(&std::fs::read_to_string(variants_path).expect("DuckShop fixture"))
            .expect("DuckShop semantic-equivalence case");
    let definition = variants["definitions"][0]["sql"].as_str().expect("SQL");
    let variant = variants["definitions"][1]["sql"]
        .as_str()
        .expect("variant SQL");
    // Independently established with the public native hash function; raw UTF-8 differs.
    let semantic = variants["semantic_hash"].as_str().expect("semantic hash");
    let record = json!({
        "event_id": "warehouse-fingerprint-1", "state_database": null, "state_schema": "main",
        "node_type": "model", "node_name": "orders", "target_database": null,
        "target_schema": "main", "target_name": "orders", "run_id": "external-command-1",
        "definition_hash": semantic, "version_hash": "version-1", "schema_fingerprint": "schema-1",
        "definition": definition, "metadata_json": "{\"audit\":1}", "ts": "2026-01-01T10:00:00Z",
    });
    let mut other = record.clone();
    other["event_id"] = json!("warehouse-fingerprint-2");
    other["run_id"] = json!("external-command-2");
    other["definition"] = json!(variant);
    let mut legacy = record.clone();
    legacy.as_object_mut().expect("record").remove("event_id");
    legacy["run_id"] = json!("legacy-command");
    let mut legacy_null = legacy.clone();
    legacy_null["event_id"] = Value::Null;
    legacy_null["ts"] = json!("2026-01-01T11:00:00+01:00");
    let mut page = vec![record.clone(); 100];
    page[1] = other;
    page[2] = legacy;
    page[3] = legacy_null.clone();
    legacy_null["ts"] = json!("2026-01-01T10:00:00.000001Z");
    page[4] = legacy_null;
    let batch = json!({"session_id": session_id, "generation": 1, "records": page});
    let typed: FingerprintBatch = serde_json::from_value(batch.clone()).expect("typed page");
    for expected in [true, false] {
        assert_eq!(
            outbound
                .sync_fingerprints("development", &typed)
                .await
                .expect("sync"),
            expected
        );
    }
    let before = mirrored_fingerprints(store).await;
    assert_eq!(
        before.len(),
        4,
        "replay and absent/NULL legacy IDs deduplicate"
    );
    let mut texts = Vec::new();
    let mut hashes = std::collections::BTreeSet::new();
    let mut source_ids = std::collections::BTreeSet::new();
    for row in &before {
        assert_eq!(row["record_json"]["definition_hash"], semantic);
        assert!(
            row["record_json"].get("definition").is_none(),
            "event holds a reference, not another body"
        );
        hashes.insert(row["definition_text_hash"].as_str().expect("hash"));
        source_ids.insert(row["source_event_id"].as_str().expect("source ID"));
        texts
            .push(String::from_utf8(decompress(&row["body"]).expect("decompress")).expect("UTF-8"));
    }
    texts.sort();
    assert_eq!(texts, [variant, definition, definition, definition]);
    assert_eq!(
        source_ids,
        std::collections::BTreeSet::from([
            "warehouse-fingerprint-1",
            "warehouse-fingerprint-2",
            "39e801c87fb3856345ca8ff4b4581edcd4439d8688c5fb399ac331948a004287",
            "ac87eafefb99f262e0304fb1f5ec891a820af7ff9cdf1e97f44306861ffbbb18",
        ])
    );
    assert_eq!(
        hashes,
        std::collections::BTreeSet::from([
            variants["definitions"][0]["text_hash"]
                .as_str()
                .expect("hash"),
            variants["definitions"][1]["text_hash"]
                .as_str()
                .expect("hash"),
        ])
    );
    let mut stale = batch.clone();
    stale["generation"] = json!(2);
    let mut oversized = batch.clone();
    oversized["records"] = json!(vec![record.clone(); 101]);
    let mut blank = batch.clone();
    blank["records"][0]["event_id"] = json!(" ");
    let mut foreign = batch.clone();
    foreign["session_id"] = json!(Uuid::now_v7());
    for (endpoint, body, status) in [
        (url.clone(), stale, StatusCode::CONFLICT),
        (url.clone(), oversized, StatusCode::BAD_REQUEST),
        (url.clone(), blank, StatusCode::BAD_REQUEST),
        (url.clone(), foreign, StatusCode::BAD_REQUEST),
        (
            url.replace(
                &outbound.location_id.to_string(),
                &Uuid::now_v7().to_string(),
            ),
            batch.clone(),
            StatusCode::FORBIDDEN,
        ),
    ] {
        assert_eq!(
            client
                .post(endpoint)
                .bearer_auth(bearer)
                .json(&body)
                .send()
                .await
                .expect("rejected page")
                .status(),
            status
        );
    }
    assert_eq!(
        client
            .post(&url)
            .json(&batch)
            .send()
            .await
            .expect("anonymous")
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let fault = TransactionFault {
        description: "fingerprint event and definition page roll back together",
        table: "sqb_warehouse_fingerprints",
        operation: "INSERT",
        condition: "WHEN NEW.record_json->>'node_name'='inventory'",
        sqlite_body: "SELECT RAISE(ABORT, 'injected fingerprint failure')",
        postgres_body: "RAISE EXCEPTION 'injected fingerprint failure'",
    };
    let mut first = record.clone();
    first["event_id"] = json!("new-fingerprint-1");
    first["definition"] = json!("select product_id from products");
    let mut second = first.clone();
    second["event_id"] = json!("new-fingerprint-2");
    second["node_name"] = json!("inventory");
    second["definition"] = json!("select inventory_id from inventory");
    let additional = json!({"session_id": session_id, "generation": 1, "records": [first, second]});
    let bodies_before = source_bodies(store).await;
    install_fault(store, &fault).await;
    assert_eq!(
        client
            .post(&url)
            .bearer_auth(bearer)
            .json(&additional)
            .send()
            .await
            .expect("faulted page")
            .status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    remove_fault(store, &fault).await;
    assert_eq!(
        mirrored_fingerprints(store).await,
        before,
        "no partial events"
    );
    assert_eq!(
        source_bodies(store).await,
        bodies_before,
        "no orphan definitions"
    );
    // Rebuild from an empty mirror, including its unshared exact-content bodies.
    let (transaction, _) = query(
        begin(store, TransactionMode::Write).await.expect("write"),
        "DELETE FROM sqb_warehouse_fingerprints",
        vec![],
    )
    .await
    .expect("wipe mirror");
    let (transaction, _) = query(transaction,
        "DELETE FROM sqb_source_texts WHERE NOT EXISTS (SELECT 1 FROM sqb_snapshot_sources s WHERE s.text_hash=sqb_source_texts.text_hash)",
        vec![]).await.expect("wipe unshared bodies");
    commit(transaction).await.expect("commit rebuild start");
    assert!(
        outbound
            .sync_fingerprints("development", &typed)
            .await
            .expect("resync")
    );
    assert_eq!(
        mirrored_fingerprints(store).await,
        before,
        "same identities, facts and exact SQL after resync"
    );
    assert_eq!(
        source_bodies(store).await,
        bodies_before,
        "same deduplicated body set"
    );
    assert_eq!(
        client
            .post(&url)
            .bearer_auth(bearer)
            .json(&additional)
            .send()
            .await
            .expect("retry faulted page")
            .json::<Value>()
            .await
            .expect("reply"),
        json!({"applied": true})
    );
    // Review ownership markers are supported warehouse rows with intentionally empty hashes/SQL.
    let mut marker = record.clone();
    marker["event_id"] = json!("warehouse-review-marker");
    for (key, value) in variants["review_marker"]
        .as_object()
        .expect("marker")
        .iter()
    {
        marker[key] = value.clone();
    }
    let marker: FingerprintBatch = serde_json::from_value(json!({
        "session_id": session_id, "generation": 1, "records": [marker],
    }))
    .expect("review marker page");
    assert!(
        outbound
            .sync_fingerprints("development", &marker)
            .await
            .expect("review marker sync")
    );
    let rows = mirrored_fingerprints(store).await;
    let marker = rows
        .iter()
        .find(|row| row["source_event_id"] == "warehouse-review-marker")
        .expect("stored marker");
    assert_eq!(marker["record_json"]["definition_hash"], "");
    assert_eq!(decompress(&marker["body"]).expect("marker definition"), b"");
    let bodies = source_bodies(store).await;
    let mut scoped = record.clone();
    scoped["state_schema"] = json!("reporting");
    let mut other_database = record.clone();
    other_database["state_database"] = json!("analytics");
    let scoped: FingerprintBatch = serde_json::from_value(json!({
        "session_id": session_id, "generation": 1, "records": [scoped, other_database],
    }))
    .expect("physical scopes");
    assert!(
        outbound
            .sync_fingerprints("development", &scoped)
            .await
            .expect("distinct physical scopes")
    );
    let preview: FingerprintBatch = serde_json::from_value(json!({
        "session_id": preview_session_id, "generation": 1, "records": [record],
    }))
    .expect("preview target");
    assert!(
        outbound
            .sync_fingerprints("preview", &preview)
            .await
            .expect("distinct target scope")
    );
    let after = mirrored_fingerprints(store).await;
    assert_eq!(
        after.len(),
        10,
        "identical source IDs in other physical/target scopes are independent"
    );
    assert_eq!(
        source_bodies(store).await,
        bodies,
        "scopes share identical definition bytes"
    );
    migrate_store(store).await.expect("warm migration");
    assert_eq!(
        mirrored_fingerprints(store).await,
        after,
        "warm migration preserves imported history"
    );
}

async fn mirrored_fingerprints(store: &ControlStore) -> Vec<Value> {
    let (transaction, rows) = query(begin(store, TransactionMode::Read).await.expect("read"),
        "SELECT f.event_id,f.source_event_id,f.definition_text_hash,f.record_json,t.body FROM sqb_warehouse_fingerprints f \
         JOIN sqb_source_texts t ON t.text_hash=f.definition_text_hash ORDER BY f.event_id", vec![]).await.expect("mirrored definitions");
    commit(transaction).await.expect("close read");
    rows.into_iter()
        .map(|row| serde_json::to_value(row).expect("row"))
        .collect()
}

async fn source_bodies(store: &ControlStore) -> Vec<Value> {
    let (transaction, rows) = query(
        begin(store, TransactionMode::Read).await.expect("read"),
        "SELECT text_hash,body FROM sqb_source_texts ORDER BY text_hash",
        vec![],
    )
    .await
    .expect("immutable bodies");
    commit(transaction).await.expect("close read");
    rows.into_iter()
        .map(|row| serde_json::to_value(row).expect("row"))
        .collect()
}
