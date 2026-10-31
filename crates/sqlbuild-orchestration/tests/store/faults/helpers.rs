//! Database-trigger fault injection at durable publication boundaries.

use sqlbuild_orchestration::store::models::ControlStore;

use crate::fault_types::TransactionFault;

pub(crate) async fn install_fault(store: &ControlStore, point: &TransactionFault) {
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
            .expect(point.description);
        return;
    };
    sqlx_core::query::query(&format!("CREATE FUNCTION sqb_test_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN {}; RETURN NEW; END $$", point.postgres_body))
        .execute(pool).await.expect("install PostgreSQL fault function");
    sqlx_core::query::query(&format!("CREATE TRIGGER sqb_test_fault BEFORE {} ON {} FOR EACH ROW {} EXECUTE FUNCTION sqb_test_fault()",
        point.operation, point.table, point.condition.replace("WHEN ", "WHEN (") + [")", ""][usize::from(point.condition.is_empty())]))
        .execute(pool).await.expect(point.description);
}

pub(crate) async fn remove_fault(store: &ControlStore, point: &TransactionFault) {
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
