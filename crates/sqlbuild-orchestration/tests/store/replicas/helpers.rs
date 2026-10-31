//! Independent database handles for multi-replica integration contracts.

use crate::store::main::open_store::open_store;
use crate::store::models::ControlStore;

pub(crate) async fn second_store(store: &ControlStore) -> ControlStore {
    let ControlStore::Postgres(pool) = store else {
        let ControlStore::Sqlite(path) = store else {
            unreachable!()
        };
        return open_store(path.to_str().expect("UTF-8 test path"))
            .await
            .expect("open independent SQLite handle");
    };
    ControlStore::Postgres(
        sqlx_postgres::PgPool::connect_with(pool.connect_options().as_ref().clone())
            .await
            .expect("open independent PostgreSQL pool"),
    )
}
