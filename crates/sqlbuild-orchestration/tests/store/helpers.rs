//! Real-database setup shared by storage, scheduling, and queue contracts.

use crate::store::main::open_store::open_store;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, TransactionMode};
use uuid::Uuid;

use crate::store::tests::store_test_types::{Backend, TestStore};

pub(crate) fn run<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("create control-store test runtime")
        .block_on(future)
}

pub(crate) async fn open_test_store(backend: Backend) -> Option<TestStore> {
    let Backend::Postgres = backend else {
        let directory = tempfile::tempdir().expect("create isolated SQLite directory");
        let store = open_store(
            directory
                .path()
                .join("control.sqlite3")
                .to_str()
                .expect("UTF-8 test path"),
        )
        .await
        .expect("open isolated SQLite store");
        return Some(TestStore {
            store,
            directory: Some(directory),
            postgres: None,
        });
    };
    let url = std::env::var("SQLBUILD_TEST_CONTROL_POSTGRES_URL").expect(
        "PostgreSQL integration requires the disposable database harness; run make test-native",
    );
    let admin = sqlx_postgres::PgPool::connect(&url)
        .await
        .expect("connect to test PostgreSQL");
    let schema = format!("sqb_test_{}", Uuid::now_v7().simple());
    sqlx_core::query::query(&format!("CREATE SCHEMA {schema}"))
        .execute(&admin)
        .await
        .expect("create isolated test schema");
    let separator = ['?', '&'][usize::from(url.contains('?'))];
    let store = open_store(&format!("{url}{separator}options=-csearch_path%3D{schema}"))
        .await
        .expect("open isolated PostgreSQL store");
    Some(TestStore {
        store,
        directory: None,
        postgres: Some((admin, schema)),
    })
}

pub(crate) async fn close_test_store(test_store: TestStore) {
    close_store(test_store.store).await;
    let Some((admin, schema)) = test_store.postgres else {
        return;
    };
    sqlx_core::query::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&admin)
        .await
        .expect("remove isolated test schema");
    admin.close().await;
    drop(test_store.directory);
}

pub(crate) async fn close_store(store: ControlStore) {
    let ControlStore::Postgres(pool) = store else {
        return;
    };
    pool.close().await;
}

pub(crate) async fn reconstruct_warehouse_predecessor(store: &ControlStore) {
    let mut transaction = begin(store, TransactionMode::Write)
        .await
        .expect("predecessor");
    for sql in [
        "DROP INDEX recall_warehouse_sources",
        "DELETE FROM sqb_schema_migrations WHERE name='source_sensors'",
        "DROP TABLE sqb_warehouse_microbatches",
        "DELETE FROM sqb_schema_migrations WHERE name='warehouse_microbatches'",
        "DROP TABLE sqb_warehouse_old_name_views",
        "DROP TABLE sqb_warehouse_migrations",
        "DELETE FROM sqb_schema_migrations WHERE name='warehouse_migrations'",
        "DROP TABLE sqb_warehouse_janitor_events",
        "DELETE FROM sqb_schema_migrations WHERE name='warehouse_janitor_events'",
        "DROP TABLE sqb_warehouse_audit_results",
        "DELETE FROM sqb_schema_migrations WHERE name='warehouse_audit_results'",
        "DROP INDEX sqb_security_audit_storage_id",
        "ALTER TABLE sqb_security_audit DROP COLUMN storage_id",
        "DELETE FROM sqb_schema_migrations WHERE name='audit_storage_order'",
        "DROP TABLE sqb_warehouse_scan_keys",
        "DROP TABLE sqb_warehouse_scan_pages",
        "DROP INDEX sqb_fingerprints_reconciliation",
        "DROP INDEX sqb_freshness_reconciliation",
        "ALTER TABLE sqb_warehouse_fingerprints DROP COLUMN state_database",
        "ALTER TABLE sqb_warehouse_fingerprints DROP COLUMN state_schema",
        "ALTER TABLE sqb_warehouse_freshness DROP COLUMN state_database",
        "ALTER TABLE sqb_warehouse_freshness DROP COLUMN state_schema",
        "DELETE FROM sqb_schema_migrations WHERE name='warehouse_reconciliation'",
    ] {
        (transaction, _) = query(transaction, sql, vec![])
            .await
            .expect("reconstruct predecessor");
    }
    commit(transaction).await.expect("commit predecessor");
}
