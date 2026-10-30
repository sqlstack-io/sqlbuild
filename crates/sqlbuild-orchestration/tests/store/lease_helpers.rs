//! Expire the persisted lease without introducing a fake control-store clock.

use sqlbuild_orchestration::store::models::ControlStore;

pub(crate) async fn expire_lease(store: &ControlStore) {
    match store {
        ControlStore::Sqlite(path) => {
            rusqlite::Connection::open(path)
                .expect("open lease expiry connection")
                .execute_batch(
                    "UPDATE sqb_scheduler_lease SET expires_at='2000-01-01T00:00:00.000000Z'",
                )
                .expect("expire SQLite scheduler lease");
        }
        ControlStore::Postgres(pool) => {
            sqlx_core::query::query(
                "UPDATE sqb_scheduler_lease SET expires_at='2000-01-01T00:00:00.000000Z'",
            )
            .execute(pool)
            .await
            .expect("expire PostgreSQL scheduler lease");
        }
    }
}
