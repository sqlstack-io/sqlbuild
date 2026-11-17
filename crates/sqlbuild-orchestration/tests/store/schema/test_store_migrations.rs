//! Numbered control-store migrations: baseline upgrades, table rebuilds, refusals and the dry run.

use crate::store::_helpers::migration::{
    MIGRATIONS, current_version, migration_plan, schema_digest,
};
use crate::store::constants::{POSTGRES_BASELINE_CHECKSUM, SQLITE_BASELINE_CHECKSUM};
use crate::store::main::migrate_store::{migrate_store, migrate_with};
use crate::store::main::plan_store_migration::plan_store_migration;
use crate::store::models::{StoreMigration, StoreMigrationPlan};
use crate::store::tests::schema::helpers::{
    BASELINE_ROWS, POPULATED_TABLES, create_unversioned_baseline, execute, migration_names,
    rebuilt_runs_table, recorded_version, schema_shape, table_rows,
};
use crate::store::tests::schema::test_types::{
    BaselineUpgradeTestCase, MigrationRefusalTestCase, StoreMigrationPlanTestCase,
};
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

const SKIPPED_RUN: &str = "INSERT INTO sqb_runs (run_id, location_id, target, snapshot_id, kind, \
    origin, root_run_id, parent_run_id, requested_by, selection_json, request_json, \
    write_set_digest, status, queued_at, node_counts_json, last_event_storage_id) VALUES \
    ('018f0000-0000-7000-8000-000000000008', '018f0000-0000-7000-8000-000000000001', 'prod', \
    '018f0000-0000-7000-8000-000000000002', 'job', 'manual', \
    '018f0000-0000-7000-8000-000000000007', '018f0000-0000-7000-8000-000000000007', 'admin', \
    '[]', '{}', 'digest-2', 'skipped', '2026-11-02T10:00:00Z', '{}', 13)";
const WIDEN_RUN_STATUS_POSTGRES: &str = "ALTER TABLE sqb_runs DROP CONSTRAINT sqb_runs_status_check; \
    ALTER TABLE sqb_runs ADD CONSTRAINT sqb_runs_status_check CHECK (status IN ('queued', \
    'launched', 'running', 'unresponsive', 'success', 'warn', 'partial', 'failed', 'cancelled', \
    'skipped'))";

#[test]
fn given_store_at_the_baseline_when_migrating_then_it_matches_a_new_store_and_keeps_its_rows() {
    let test_cases = [
        BaselineUpgradeTestCase {
            description: "SQLite store",
            backend: Backend::Sqlite,
            baseline_checksum: SQLITE_BASELINE_CHECKSUM,
        },
        BaselineUpgradeTestCase {
            description: "PostgreSQL store",
            backend: Backend::Postgres,
            baseline_checksum: POSTGRES_BASELINE_CHECKSUM,
        },
    ];
    for test_case in test_cases {
        run(async {
            let (Some(upgraded), Some(created)) = (
                open_test_store(test_case.backend).await,
                open_test_store(test_case.backend).await,
            ) else {
                return;
            };
            create_unversioned_baseline(&upgraded.store, test_case.baseline_checksum).await;
            let before = table_rows(&upgraded.store, &POPULATED_TABLES).await;

            migrate_store(&upgraded.store)
                .await
                .expect("upgrade the baseline store");
            migrate_store(&created.store).await.expect("create a store");

            assert_eq!(
                schema_shape(&upgraded.store).await,
                schema_shape(&created.store).await,
                "{}: a schema change needs a numbered migration that brings the baseline to it",
                test_case.description
            );
            assert_eq!(
                table_rows(&upgraded.store, &POPULATED_TABLES).await,
                before,
                "{}: rows changed",
                test_case.description
            );
            assert_eq!(
                recorded_version(&upgraded.store).await,
                Some(current_version(MIGRATIONS)),
                "{}: recorded version",
                test_case.description
            );
            close_test_store(upgraded).await;
            close_test_store(created).await;
        });
    }
}

#[test]
fn given_a_migration_rebuilding_a_referenced_table_when_migrating_then_rows_and_references_survive()
{
    let backends = [Backend::Sqlite, Backend::Postgres];
    for backend in backends {
        run(async {
            let Some(fixture) = open_test_store(backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_with(store, &[]).await.expect("create the store");
            execute(store, BASELINE_ROWS).await;
            let before = table_rows(store, &POPULATED_TABLES).await;
            let shape_before: Vec<String> = schema_shape(store)
                .await
                .into_iter()
                .filter(|item| !item.contains("'cancelled'"))
                .collect();
            let migration = StoreMigration {
                name: "widen_run_status",
                sqlite: rebuilt_runs_table(store).await,
                postgres: WIDEN_RUN_STATUS_POSTGRES,
            };

            migrate_with(store, &[migration])
                .await
                .expect("rebuild sqb_runs");
            execute(store, SKIPPED_RUN).await;

            let shape_after: Vec<String> = schema_shape(store)
                .await
                .into_iter()
                .filter(|item| !item.contains("'cancelled'"))
                .collect();
            let mut rows = table_rows(store, &POPULATED_TABLES).await;
            let runs = rows.pop().expect("sqb_runs rows");
            assert_eq!(
                (rows, runs.len()),
                (before[..POPULATED_TABLES.len() - 1].to_vec(), 2),
                "{backend:?}: rows after the rebuild"
            );
            assert_eq!(
                shape_after, shape_before,
                "{backend:?}: other schema objects"
            );
            assert_eq!(
                recorded_version(store).await,
                Some(2),
                "{backend:?}: version"
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_a_store_this_build_cannot_upgrade_when_migrating_then_it_is_refused_unchanged() {
    let test_cases = [
        MigrationRefusalTestCase {
            description: "SQLite store from a newer build",
            backend: Backend::Sqlite,
            tamper: "UPDATE sqb_schema_version SET version = 99",
            expected_fragments: vec!["schema is version 99, newer than this build's version"],
        },
        MigrationRefusalTestCase {
            description: "SQLite store older than the baseline",
            backend: Backend::Sqlite,
            tamper: "UPDATE sqb_schema_version SET checksum = 'unknown', version = NULL",
            expected_fragments: vec!["older than version 1", "rm -f '"],
        },
        MigrationRefusalTestCase {
            description: "SQLite store whose schema changed without a migration",
            backend: Backend::Sqlite,
            tamper: "UPDATE sqb_schema_version SET checksum = 'unknown'",
            expected_fragments: vec![
                "doesn't match this build's, although both are version",
                "rm -f '",
            ],
        },
        MigrationRefusalTestCase {
            description: "PostgreSQL store from a newer build",
            backend: Backend::Postgres,
            tamper: "UPDATE sqb_schema_version SET version = 99",
            expected_fragments: vec!["schema is version 99, newer than this build's version"],
        },
        MigrationRefusalTestCase {
            description: "PostgreSQL store older than the baseline",
            backend: Backend::Postgres,
            tamper: "UPDATE sqb_schema_version SET checksum = 'unknown', version = NULL",
            expected_fragments: vec!["older than version 1", "dropdb -h '"],
        },
        MigrationRefusalTestCase {
            description: "PostgreSQL store whose schema changed without a migration",
            backend: Backend::Postgres,
            tamper: "UPDATE sqb_schema_version SET checksum = 'unknown'",
            expected_fragments: vec![
                "doesn't match this build's, although both are version",
                "dropdb -h '",
            ],
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("create the store");
            execute(store, test_case.tamper).await;
            let version_before = recorded_version(store).await;

            let error = migrate_store(store)
                .await
                .expect_err("refuse the store")
                .to_string();

            let missing: Vec<&str> = test_case
                .expected_fragments
                .iter()
                .copied()
                .filter(|fragment| !error.contains(fragment))
                .collect();
            assert_eq!(
                missing,
                Vec::<&str>::new(),
                "{}: {error}",
                test_case.description
            );
            assert_eq!(
                recorded_version(store).await,
                version_before,
                "{}: store changed",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_a_store_when_planning_its_migration_then_the_plan_is_reported_without_changing_it() {
    let test_cases = [
        StoreMigrationPlanTestCase {
            description: "SQLite store",
            backend: Backend::Sqlite,
            baseline_checksum: SQLITE_BASELINE_CHECKSUM,
        },
        StoreMigrationPlanTestCase {
            description: "PostgreSQL store",
            backend: Backend::Postgres,
            baseline_checksum: POSTGRES_BASELINE_CHECKSUM,
        },
    ];
    for test_case in test_cases {
        run(async {
            let (Some(empty), Some(baseline)) = (
                open_test_store(test_case.backend).await,
                open_test_store(test_case.backend).await,
            ) else {
                return;
            };
            create_unversioned_baseline(&baseline.store, test_case.baseline_checksum).await;

            let plans = (
                plan_store_migration(&empty.store)
                    .await
                    .expect("plan an empty store"),
                plan_store_migration(&baseline.store)
                    .await
                    .expect("plan the baseline store"),
                recorded_version(&baseline.store).await,
            );
            migrate_store(&baseline.store)
                .await
                .expect("upgrade the baseline store");
            let current = plan_store_migration(&baseline.store)
                .await
                .expect("plan the upgraded store");
            let current_version = current_version(MIGRATIONS);
            let mut with_pending: Vec<StoreMigration> = MIGRATIONS.to_vec();
            with_pending.push(StoreMigration {
                name: "widen_run_status",
                sqlite: "",
                postgres: "",
            });
            let pending = migration_plan(
                &baseline.store,
                Some((
                    schema_digest(&baseline.store).as_str(),
                    Some(current_version),
                )),
                &with_pending,
            )
            .expect("plan a pending migration");

            assert_eq!(
                (plans, current, pending),
                (
                    (
                        StoreMigrationPlan::Create {
                            version: current_version,
                        },
                        StoreMigrationPlan::Upgrade {
                            from: 1,
                            to: current_version,
                            names: migration_names(MIGRATIONS, 1),
                        },
                        None,
                    ),
                    StoreMigrationPlan::Current {
                        version: current_version,
                    },
                    StoreMigrationPlan::Upgrade {
                        from: current_version,
                        to: current_version + 1,
                        names: vec![format!("{:04}_widen_run_status", current_version + 1)],
                    },
                ),
                "{}",
                test_case.description
            );
            close_test_store(empty).await;
            close_test_store(baseline).await;
        });
    }
}
