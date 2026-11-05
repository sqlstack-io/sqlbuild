//! Real-database setup shared by storage, scheduling, and queue contracts.

use crate::store::main::open_store::open_store;
use crate::store::models::ControlStore;
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
    // Writer advisory locks are database-wide, so schemas alone do not isolate contracts.
    sqlx_core::query::query(&format!("CREATE DATABASE {schema} TEMPLATE template0"))
        .execute(&admin)
        .await
        .expect("create isolated test database");
    let mut isolated_url = reqwest::Url::parse(&url).expect("test PostgreSQL URL");
    isolated_url.set_path(&schema);
    let url = isolated_url.as_str();
    let setup = sqlx_postgres::PgPool::connect(url)
        .await
        .expect("connect to isolated test database");
    sqlx_core::query::query(&format!("CREATE SCHEMA {schema}"))
        .execute(&setup)
        .await
        .expect("create isolated test schema");
    setup.close().await;
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
    sqlx_core::query::query(&format!("DROP DATABASE {schema}"))
        .execute(&admin)
        .await
        .expect("remove isolated test database");
    admin.close().await;
    drop(test_store.directory);
}

pub(crate) async fn close_store(store: ControlStore) {
    let ControlStore::Postgres(pool) = store else {
        return;
    };
    pool.close().await;
}
