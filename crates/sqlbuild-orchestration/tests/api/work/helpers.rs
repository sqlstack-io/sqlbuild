//! Fault injection changes only disposable test stores, never product time or lease semantics.

use crate::store::models::ControlStore;

pub(crate) async fn execute(store: &ControlStore, sql: &str) {
    let ControlStore::Postgres(pool) = store else {
        let ControlStore::Sqlite(path, _) = store else {
            unreachable!("SQLite store")
        };
        rusqlite::Connection::open(path)
            .expect("fault connection")
            .execute_batch(sql)
            .expect("inject SQLite fault");
        return;
    };
    sqlx_core::raw_sql::raw_sql(sql)
        .execute(pool)
        .await
        .expect("inject PostgreSQL fault");
}
