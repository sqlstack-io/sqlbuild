//! Real-database setup shared by storage, scheduling, and queue contracts.

use sqlbuild_orchestration::store::main::open_store::open_store;
use sqlbuild_orchestration::store::models::ControlStore;
use uuid::Uuid;

use crate::store_test_types::{Backend, TestStore};

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
    let Ok(url) = std::env::var("SQLBUILD_TEST_CONTROL_POSTGRES_URL") else {
        assert!(
            std::env::var_os("SQLBUILD_REQUIRE_POSTGRES_TESTS").is_none(),
            "PostgreSQL test URL is required"
        );
        eprintln!("PostgreSQL integration requires SQLBUILD_TEST_CONTROL_POSTGRES_URL");
        return None;
    };
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
