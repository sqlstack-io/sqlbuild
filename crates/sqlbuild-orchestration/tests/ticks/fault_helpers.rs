use sqlbuild_orchestration::store::models::ControlStore;

use crate::test_types::TickFaultTestCase;

pub(crate) async fn install_fault(store: &ControlStore, point: &TickFaultTestCase) {
    let ControlStore::Postgres(pool) = store else {
        let ControlStore::Sqlite(path) = store else {
            unreachable!()
        };
        rusqlite::Connection::open(path)
            .expect("fault connection")
            .execute_batch(&format!(
                "CREATE TRIGGER sqb_test_fault BEFORE {} ON {} {} BEGIN {}; END;",
                point.operation, point.table, point.condition, point.sqlite_body
            ))
            .expect("install SQLite transaction fault");
        return;
    };
    sqlx_core::query::query(&format!("CREATE FUNCTION sqb_test_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN {}; RETURN NEW; END $$", point.postgres_body))
        .execute(pool).await.expect("install PostgreSQL fault function");
    sqlx_core::query::query(&format!("CREATE TRIGGER sqb_test_fault BEFORE {} ON {} FOR EACH ROW {} EXECUTE FUNCTION sqb_test_fault()",
        point.operation, point.table, point.condition.replace("WHEN ", "WHEN (") + &[")", ""][usize::from(point.condition.is_empty())]))
        .execute(pool).await.expect("install PostgreSQL transaction fault");
}

pub(crate) async fn remove_fault(store: &ControlStore, point: &TickFaultTestCase) {
    let ControlStore::Postgres(pool) = store else {
        let ControlStore::Sqlite(path) = store else {
            unreachable!()
        };
        rusqlite::Connection::open(path)
            .expect("fault connection")
            .execute_batch("DROP TRIGGER sqb_test_fault")
            .expect("remove SQLite transaction fault");
        return;
    };
    sqlx_core::query::query(&format!("DROP TRIGGER sqb_test_fault ON {}", point.table))
        .execute(pool)
        .await
        .expect("remove PostgreSQL fault trigger");
    sqlx_core::query::query("DROP FUNCTION sqb_test_fault()")
        .execute(pool)
        .await
        .expect("remove PostgreSQL fault function");
}

pub(crate) async fn expire_lease(store: &ControlStore) {
    let ControlStore::Postgres(pool) = store else {
        let ControlStore::Sqlite(path) = store else {
            unreachable!()
        };
        rusqlite::Connection::open(path)
            .expect("fault connection")
            .execute_batch(
                "UPDATE sqb_scheduler_lease SET expires_at='2000-01-01T00:00:00.000000Z'",
            )
            .expect("expire SQLite lease");
        return;
    };
    sqlx_core::query::query(
        "UPDATE sqb_scheduler_lease SET expires_at='2000-01-01T00:00:00.000000Z'",
    )
    .execute(pool)
    .await
    .expect("expire PostgreSQL lease");
}
