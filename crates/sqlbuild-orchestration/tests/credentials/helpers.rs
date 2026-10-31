use sqlbuild_orchestration::store::models::ControlStore;

pub(crate) async fn location_state(store: &ControlStore) -> (i64, i64) {
    let sql = "SELECT (SELECT COUNT(*) FROM sqb_locations), \
               (SELECT COUNT(*) FROM sqb_security_audit WHERE action='location.created')";
    let ControlStore::Postgres(pool) = store else {
        let ControlStore::Sqlite(path) = store else {
            unreachable!()
        };
        return rusqlite::Connection::open(path)
            .expect("inspect locations")
            .query_row(sql, [], |row| Ok((row.get(0)?, row.get(1)?)))
            .expect("location and audit counts");
    };
    sqlx_core::query_as::query_as(sql)
        .fetch_one(pool)
        .await
        .expect("location and audit counts")
}

pub(crate) async fn stored_hashes(store: &ControlStore) -> Vec<String> {
    let ControlStore::Postgres(pool) = store else {
        let ControlStore::Sqlite(path) = store else {
            unreachable!()
        };
        let connection = rusqlite::Connection::open(path).expect("inspect durable tokens");
        return connection
            .prepare("SELECT secret_hash FROM sqb_access_tokens")
            .expect("token hashes")
            .query_map([], |row| row.get(0))
            .expect("read token hashes")
            .collect::<Result<_, _>>()
            .expect("hashes");
    };
    sqlx_core::query_scalar::query_scalar("SELECT secret_hash FROM sqb_access_tokens")
        .fetch_all(pool)
        .await
        .expect("read durable token hashes")
}
