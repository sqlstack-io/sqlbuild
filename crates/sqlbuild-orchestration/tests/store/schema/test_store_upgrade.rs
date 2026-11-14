//! Opening a store created before the foreign-key indexes adds them in place and keeps history.

use std::collections::BTreeSet;

use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::migrate_store::migrate_store;
use crate::store::main::query::query;
use crate::store::models::TransactionMode;
use crate::store::tests::schema::helpers::text;
use crate::store::tests::schema::test_types::StoreUpgradeTestCase;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

const FOREIGN_KEY_INDEXES: [&str; 10] = [
    "sqb_jobs_snapshot",
    "sqb_location_work_session",
    "sqb_location_work_snapshot",
    "sqb_role_assignments_location",
    "sqb_run_requests_run",
    "sqb_runs_rerun_of",
    "sqb_runs_trigger",
    "sqb_triggers_removed",
    "sqb_triggers_snapshot",
    "sqb_warehouse_fingerprints_text",
];

#[test]
fn given_store_from_previous_schema_when_opening_then_indexes_are_added_and_history_is_kept() {
    let test_cases = [
        StoreUpgradeTestCase {
            description: "SQLite store",
            backend: Backend::Sqlite,
            list_indexes: "SELECT name FROM sqlite_master WHERE type = 'index'",
            expected_indexes: FOREIGN_KEY_INDEXES.to_vec(),
        },
        StoreUpgradeTestCase {
            description: "PostgreSQL store",
            backend: Backend::Postgres,
            list_indexes: "SELECT indexname AS name FROM pg_indexes \
                WHERE schemaname = current_schema()",
            expected_indexes: FOREIGN_KEY_INDEXES.to_vec(),
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("create store");
            let mut transaction = begin(store, TransactionMode::Write)
                .await
                .expect("begin downgrade");
            for index in FOREIGN_KEY_INDEXES {
                (transaction, _) = query(transaction, &format!("DROP INDEX {index}"), vec![])
                    .await
                    .expect("drop index to recreate the previous schema");
            }
            let (transaction, before) = query(
                transaction,
                "SELECT installation_id FROM sqb_installation",
                vec![],
            )
            .await
            .expect("read installation before upgrade");
            commit(transaction).await.expect("commit downgrade");

            migrate_store(store)
                .await
                .expect("open store from the previous schema");

            let transaction = begin(store, TransactionMode::Read)
                .await
                .expect("begin read");
            let (transaction, after) = query(
                transaction,
                "SELECT installation_id FROM sqb_installation",
                vec![],
            )
            .await
            .expect("read installation after upgrade");
            let (transaction, indexes) = query(transaction, test_case.list_indexes, vec![])
                .await
                .expect("list indexes");
            commit(transaction).await.expect("finish read");
            let present: BTreeSet<String> = indexes.iter().map(|row| text(row, "name")).collect();
            let added: Vec<&str> = test_case
                .expected_indexes
                .iter()
                .copied()
                .filter(|index| present.contains(*index))
                .collect();
            assert_eq!(
                added, test_case.expected_indexes,
                "{}: indexes after upgrade",
                test_case.description
            );
            assert_eq!(
                (before.len(), &before),
                (1, &after),
                "{}: installation history changed",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
