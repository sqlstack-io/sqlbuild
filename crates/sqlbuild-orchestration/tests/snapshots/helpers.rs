//! Real control-store integration coverage; both engines exercise the same public contract.

use std::collections::BTreeMap;

use serde_json::json;
use sqlbuild_orchestration::snapshots::main::current_snapshot::current_snapshot;
use sqlbuild_orchestration::snapshots::main::publish_snapshot::publish_snapshot;
use sqlbuild_orchestration::snapshots::models::{SnapshotInput, SnapshotNode};
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::store::main::migrate_store::migrate_store;
use sqlbuild_orchestration::store::main::open_store::open_store;
use sqlbuild_orchestration::store::models::ControlStore;
use uuid::Uuid;

use crate::location_helpers::create_location;
use crate::replica_helpers::second_store;
use crate::store_helpers::{close_store, close_test_store, open_test_store};
use crate::store_test_types::Backend;
use crate::test_types::SchemaRejectionCase;

fn snapshot(location_id: Uuid) -> SnapshotInput {
    SnapshotInput {
        location_id,
        target: "development".into(),
        generation: 1,
        git_revision: None,
        git_dirty: true,
        git_author: None,
        image_id: None,
        environment_hash: "synthetic-environment".into(),
        sqb_version: env!("CARGO_PKG_VERSION").into(),
        nodes: BTreeMap::from([
            (
                "customers".into(),
                SnapshotNode {
                    kind: "model".into(),
                    definition: json!({"materialization": "table", "columns": ["customer_id"]}),
                    raw_sql: Some("select 1 as customer_id".into()),
                    compiled_sql: Some("select 1 as customer_id".into()),
                    python_source: None,
                },
            ),
            (
                "orders".into(),
                SnapshotNode {
                    kind: "model".into(),
                    definition: json!({"dependencies": ["customers"], "materialization": "view"}),
                    raw_sql: Some("select customer_id from ref('customers')".into()),
                    compiled_sql: Some("select customer_id from main.customers".into()),
                    python_source: Some("# display only: orders".into()),
                },
            ),
        ]),
        documents: BTreeMap::from([("lineage".into(), json!({"orders": ["customers"]}))]),
    }
}

async fn exercise_snapshot_contract(store: &ControlStore) -> i64 {
    let second = second_store(store).await;
    let (first, second_migration) = tokio::join!(migrate_store(store), migrate_store(&second));
    first.expect("first migration succeeds");
    second_migration.expect("concurrent migration succeeds");
    let location = create_location(store, "orders", "test-admin")
        .await
        .expect("create test location");
    assert!(
        current_snapshot(store, location, "development")
            .await
            .expect("read empty target")
            .is_none()
    );
    let mut input = snapshot(location);
    let initial = publish_snapshot(store, &input)
        .await
        .expect("publish initial snapshot");
    let loaded = current_snapshot(store, location, "development")
        .await
        .expect("read initial snapshot")
        .expect("initial snapshot exists");
    assert_eq!(loaded.record, initial);
    assert_eq!(loaded.nodes, input.nodes);
    assert_eq!(loaded.documents, input.documents);
    assert_eq!(
        loaded
            .changes
            .iter()
            .map(|change| (change.subject_key.as_str(), change.change_kind.as_str()))
            .collect::<Vec<_>>(),
        [("customers", "added"), ("orders", "added")]
    );

    // Container insertion order and the generation do not affect the content address.
    input.generation = 2;
    input
        .nodes
        .get_mut("orders")
        .expect("orders node exists")
        .definition = json!({"materialization": "view", "dependencies": ["customers"]});
    let unchanged = publish_snapshot(store, &input)
        .await
        .expect("publish reordered definition");
    assert_eq!(unchanged.manifest_digest, initial.manifest_digest);
    assert!(
        current_snapshot(store, location, "development")
            .await
            .expect("read unchanged snapshot")
            .expect("unchanged snapshot exists")
            .changes
            .is_empty()
    );
    assert!(matches!(
        publish_snapshot(store, &input).await,
        Err(ControlError::StaleGeneration)
    ));

    input.generation = 3;
    input
        .nodes
        .get_mut("orders")
        .expect("orders node exists")
        .compiled_sql = Some("select customer_id + 5 from main.customers".into());
    let changed = publish_snapshot(store, &input)
        .await
        .expect("publish changed SQL");
    assert_ne!(changed.manifest_digest, unchanged.manifest_digest);
    let loaded = current_snapshot(store, location, "development")
        .await
        .expect("read changed snapshot")
        .expect("changed snapshot exists");
    assert_eq!(loaded.nodes, input.nodes);
    assert_eq!(loaded.changes.len(), 1);
    assert_eq!(loaded.changes[0].subject_key, "orders");
    assert_eq!(loaded.changes[0].change_kind, "changed");
    assert_ne!(loaded.changes[0].before_hash, loaded.changes[0].after_hash);

    // One target's generation does not suppress another target's first publication.
    input.target = "testing".into();
    input.generation = 1;
    publish_snapshot(store, &input)
        .await
        .expect("publish another target");
    assert_eq!(
        current_snapshot(store, location, "development")
            .await
            .expect("read development target")
            .expect("development target exists")
            .record
            .generation,
        3
    );
    assert_eq!(
        current_snapshot(store, location, "testing")
            .await
            .expect("read testing target")
            .expect("testing target exists")
            .record
            .generation,
        1
    );

    input.target = "development".into();
    input.generation = 4;
    input.nodes.remove("orders");
    publish_snapshot(store, &input)
        .await
        .expect("publish node removal");
    let loaded = current_snapshot(store, location, "development")
        .await
        .expect("read node removal")
        .expect("snapshot exists after removal");
    assert_eq!(loaded.changes.len(), 1);
    assert_eq!(loaded.changes[0].change_kind, "removed");
    assert_eq!(loaded.changes[0].subject_key, "orders");
    assert!(loaded.changes[0].after_hash.is_none());

    // The newer compiler wins regardless of which publisher takes the transaction lock first.
    let mut older = input.clone();
    older.generation = 5;
    input.generation = 6;
    input
        .nodes
        .get_mut("customers")
        .expect("customers node exists")
        .compiled_sql = Some("select 6 as customer_id".into());
    let (old_result, new_result) = tokio::join!(
        publish_snapshot(store, &older),
        publish_snapshot(&second, &input)
    );
    assert!(old_result.is_ok() || matches!(old_result, Err(ControlError::StaleGeneration)));
    new_result.expect("new generation wins");
    let loaded = current_snapshot(store, location, "development")
        .await
        .expect("read winning snapshot")
        .expect("winning snapshot exists");
    assert_eq!(loaded.record.generation, 6);
    assert_eq!(loaded.nodes, input.nodes);
    close_store(second).await;
    loaded.record.generation
}

pub(super) async fn snapshot_contract(backend: Backend) -> Option<i64> {
    let test_store = open_test_store(backend).await?;
    let generation = exercise_snapshot_contract(&test_store.store).await;
    verify_sqlite_deduplication(&test_store.store);
    close_test_store(test_store).await;
    Some(generation)
}

fn verify_sqlite_deduplication(store: &ControlStore) {
    let ControlStore::Sqlite(path) = store else {
        return;
    };
    let connection = rusqlite::Connection::open(path).expect("inspect control store");
    let definitions: i64 = connection
        .query_row("SELECT count(*) FROM sqb_node_definitions", [], |row| {
            row.get(0)
        })
        .expect("count immutable definitions");
    // Two initial nodes, one changed source, and one changed customer; unchanged targets reuse them.
    assert_eq!(definitions, 4);
    let sources: i64 = connection
        .query_row("SELECT count(*) FROM sqb_source_texts", [], |row| {
            row.get(0)
        })
        .expect("count shared sources");
    assert_eq!(sources, 6);
}

pub(super) async fn sqlite_failed_publish_and_schema_floor() -> i64 {
    let directory = tempfile::tempdir().expect("create test directory");
    let path = directory.path().join("control.sqlite3");
    let store = open_store(path.to_str().expect("UTF-8 test path"))
        .await
        .expect("open test store");
    migrate_store(&store).await.expect("migrate test store");
    let location = create_location(&store, "orders", "test-admin")
        .await
        .expect("create test location");
    let mut input = snapshot(location);
    let good = publish_snapshot(&store, &input)
        .await
        .expect("publish good snapshot");
    let connection = rusqlite::Connection::open(&path).expect("open fault injection connection");
    connection
        .execute_batch(
            "DROP TABLE sqb_location_work_results; DROP TABLE sqb_location_work; \
             DROP TABLE sqb_location_instances; DELETE FROM sqb_schema_migrations WHERE version>=2",
        )
        .expect("reconstruct the previous schema without changing existing facts");
    migrate_store(&store)
        .await
        .expect("upgrade the previous schema");
    migrate_store(&store)
        .await
        .expect("repeat the migration without changes");
    assert_eq!(
        current_snapshot(&store, location, "development")
            .await
            .expect("read upgraded store")
            .expect("snapshot survives upgrade")
            .record,
        good
    );
    // Fail after immutable bodies were inserted but before switching the current pointer.
    connection.execute_batch("CREATE TRIGGER fail_snapshot_switch BEFORE UPDATE ON sqb_current_snapshots BEGIN SELECT RAISE(ABORT, 'synthetic publication failure'); END").expect("install publication failure");
    input.generation = 2;
    input
        .nodes
        .get_mut("orders")
        .expect("orders node exists")
        .compiled_sql = Some("select 99".into());
    assert!(publish_snapshot(&store, &input).await.is_err());
    assert_eq!(
        current_snapshot(&store, location, "development")
            .await
            .expect("read preserved snapshot")
            .expect("last good snapshot exists")
            .record,
        good
    );
    let count: i64 = connection
        .query_row("SELECT count(*) FROM sqb_snapshots", [], |row| row.get(0))
        .expect("count committed snapshots");
    assert_eq!(count, 1);
    connection
        .execute_batch("DROP TRIGGER fail_snapshot_switch")
        .expect("remove injected failure");
    let recovered = publish_snapshot(&store, &input)
        .await
        .expect("retry publication");
    for case in [
        SchemaRejectionCase {
            description: "minimum server version is newer",
            update_sql: "UPDATE sqb_schema_migrations SET min_server_version='9999.0.0'",
        },
        SchemaRejectionCase {
            description: "migration checksum changed",
            update_sql: "UPDATE sqb_schema_migrations SET min_server_version='0.0.0', checksum='different'",
        },
        SchemaRejectionCase {
            description: "migration version is unknown",
            update_sql: "UPDATE sqb_schema_migrations SET version=999 WHERE version=2",
        },
    ] {
        connection
            .execute_batch(case.update_sql)
            .expect("apply synthetic schema incompatibility");
        assert!(
            matches!(migrate_store(&store).await, Err(ControlError::Schema(_))),
            "{}",
            case.description
        );
    }
    recovered.generation
}
