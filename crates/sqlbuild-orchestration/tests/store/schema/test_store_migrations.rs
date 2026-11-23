//! Numbered control-store migrations: baseline upgrades, table rebuilds, refusals and the dry run.

use crate::credentials::main::service_keys::authenticate_service_key::authenticate_service_key;
use crate::credentials::models::ServiceKeyCheck;
use crate::store::constants::{INDEXES, SCHEMA};
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::schema::helpers::{
    BASELINE_ROWS, POPULATED_TABLES, POSTGRES_STORE, SQLITE_STORE, create_unversioned_baseline,
    execute, migration_names, query_items, rebuilt_runs_table, recorded_version, schema_shape,
    service_account_key, table_rows, without_added_columns,
};
use crate::store::tests::schema::test_types::{
    BaselineUpgradeTestCase, BrokenMigrationTestCase, LiveWorkUpgradeTestCase,
    MigrationRefusalTestCase, PinnedChecksumTestCase, ServiceAccountUpgradeTestCase,
    StoreMigrationPlanTestCase, TableRebuildTestCase,
};
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store_migrations::_helpers::migration::{
    MIGRATIONS, current_version, migration_plan, schema_digest, schema_text_digest,
};
use crate::store_migrations::_helpers::migration_run::migrate_with;
use crate::store_migrations::_helpers::recall_schema::recall_schema;
use crate::store_migrations::main::plan_store_migration::plan_store_migration;
use crate::store_migrations::models::{StoreMigration, StoreMigrationPlan};

const SKIPPED_RUN: &str = "INSERT INTO sqb_runs (run_id, location_id, target, snapshot_id, kind, \
    origin, root_run_id, parent_run_id, requested_by, selection_json, request_json, \
    write_set_digest, status, queued_at, node_counts_json, last_event_storage_id) VALUES \
    ('018f0000-0000-7000-8000-000000000008', '018f0000-0000-7000-8000-000000000001', 'prod', \
    '018f0000-0000-7000-8000-000000000002', 'job', 'manual', \
    '018f0000-0000-7000-8000-000000000007', '018f0000-0000-7000-8000-000000000007', 'admin', \
    '[]', '{}', 'digest-2', 'skipped', '2026-11-02T10:00:00Z', '{}', 13)";
const SQLITE_CHECKSUMS: &[(i64, &str)] = &[
    (
        1,
        "6d0e581f453370ec5156053d644dea3b22b19332cee588eb69ac3141ada5a76b",
    ),
    (
        2,
        "4d4e3aa56b3a39d8550f691ae12fe463187b4f4cdeacc66ca94803701cab5e0a",
    ),
    (
        3,
        "734163e22863ff86fbcf1329540736f279806d4ce5ea04f6d6ecb1703b3468c8",
    ),
    // 0004 rewrites stored job definitions and leaves the schema as it is.
    (
        4,
        "734163e22863ff86fbcf1329540736f279806d4ce5ea04f6d6ecb1703b3468c8",
    ),
    (
        5,
        "48d191962e39b22a3b95b9495da6371067a3fd3cfc51192bcfe0ac0192edcba2",
    ),
    (
        6,
        "efeb5ed2f8bd2d191ec6848427c47201892b299d92c92ca7d0fcdacd0a49d69e",
    ),
    (
        7,
        "56f505ab32cf6ce3e694461516616d390c008ff5622af56001c3443028b230d4",
    ),
    (
        8,
        "caad867ae87f86bedb52d51330f390aba223630f147b86353f899a189160e245",
    ),
    (
        9,
        "53e0dbe5dd3eb23174e46480e8c805d9ede865a65f132c545f989603f0a93695",
    ),
    (
        10,
        "ff56abd4e244e08469861e37cef2ec8a460c3bc9ab2cb42d4e12c5a0cc35f3bb",
    ),
];
const POSTGRES_CHECKSUMS: &[(i64, &str)] = &[
    (
        1,
        "2a81e526eb1181eeab0b897e8b3f48ae2dd9425d85cb788874670d942bbc76c4",
    ),
    (
        2,
        "99e13d88687d6e590e47149200a0d1cb03c0ff285a95339da40b76cba99e2a21",
    ),
    (
        3,
        "b1250935a5adf1d4691836a2125bb57360c1ff0a2a296a77e4d11bf6a80a99d9",
    ),
    // 0004 rewrites stored job definitions and leaves the schema as it is.
    (
        4,
        "b1250935a5adf1d4691836a2125bb57360c1ff0a2a296a77e4d11bf6a80a99d9",
    ),
    (
        5,
        "5a86ec47fce6ba3ced0badeb7ddb5b54d5bd2408fbe917b5c11391d362387ebb",
    ),
    (
        6,
        "b6400df03f6b2bf630f7a26b4f1f070fd12dfe5097fe471cb175a65b002a0c08",
    ),
    (
        7,
        "3279facbd626157c799e4ceea29d61572b1f3af2ece51281e55b79dfc612e1e5",
    ),
    (
        8,
        "716de0aa5611c1a57238550853888daa3e84cd10b7d1a00297766e17b876b938",
    ),
    (
        9,
        "36e079cd622544e221ddfb9bc8837d8030586f7628aaa77da3c1763abb4c4442",
    ),
    (
        10,
        "05cc81606b55e1e3c7a4482a34c24a94bcaac6bfe74e0dbc4680a9ba92da4eb1",
    ),
];
/// A version 1 store mid-run: a child run holding a concurrency slot, a jobs document and an audit result.
const VERSION_1_LIVE_WORK: &str = "ALTER TABLE sqb_schema_version ADD COLUMN version BIGINT; \
    UPDATE sqb_schema_version SET version = 1; \
    INSERT INTO sqb_runs (run_id, location_id, target, snapshot_id, kind, origin, job_key, \
    parent_run_id, root_run_id, requested_by, selection_json, request_json, write_set_digest, status, \
    queued_at, node_counts_json, last_event_storage_id) VALUES \
    ('018f0000-0000-7000-8000-000000000009', '018f0000-0000-7000-8000-000000000001', 'prod', \
    '018f0000-0000-7000-8000-000000000002', 'job', 'manual', 'daily_orders', \
    '018f0000-0000-7000-8000-000000000007', '018f0000-0000-7000-8000-000000000007', 'admin', '[]', \
    '{}', 'digest-3', 'running', '2026-11-01T11:00:00Z', '{}', 14); \
    INSERT INTO sqb_concurrency_slots (group_key, slot, run_id) VALUES \
    ('orders', 1, '018f0000-0000-7000-8000-000000000009'); \
    INSERT INTO sqb_documents (document_hash, body) VALUES ('document-1', ''); \
    INSERT INTO sqb_snapshot_documents (snapshot_id, kind, document_hash) VALUES \
    ('018f0000-0000-7000-8000-000000000002', 'jobs', 'document-1'); \
    INSERT INTO sqb_warehouse_audit_results (event_id, location_id, target, observed_at, state_schema, \
    audit_name, outcome, record_json, received_at) VALUES ('018f0000-0000-7000-8000-00000000000a', \
    '018f0000-0000-7000-8000-000000000001', 'prod', '2026-11-01T10:03:00Z', 'analytics', \
    'orders_present', 'error', '{}', '2026-11-01T10:04:00Z')";
/// Writes version 2 allows: a skipped run, a second slot for one run and a connection document.
const VERSION_2_WRITES: &str = "INSERT INTO sqb_concurrency_slots (group_key, slot, run_id) VALUES \
    ('orders', 2, '018f0000-0000-7000-8000-000000000009'); \
    INSERT INTO sqb_snapshot_documents (snapshot_id, kind, document_hash) VALUES \
    ('018f0000-0000-7000-8000-000000000002', 'connection', 'document-1')";
const RUNS_AFTER_UPGRADE: &str = "SELECT CAST(run_id AS TEXT) || ' ' || status || ' ' \
    || CAST(ignored_audit_failures AS TEXT) AS item FROM sqb_runs ORDER BY run_id";
const AUDITS_AFTER_UPGRADE: &str = "SELECT audit_name || CASE WHEN ignored THEN ' ignored' ELSE ' counted' END \
    AS item FROM sqb_warehouse_audit_results UNION ALL SELECT 'view rows ' || CAST(COUNT(ignored) AS TEXT) \
    FROM audit_results";
const JOINED_REFERENCES: &str = "SELECT 'run nodes ' || CAST(COUNT(*) AS TEXT) AS item FROM sqb_run_nodes \
    JOIN sqb_runs USING (run_id) UNION ALL SELECT 'slots ' || CAST(COUNT(*) AS TEXT) \
    FROM sqb_concurrency_slots JOIN sqb_runs USING (run_id) UNION ALL SELECT 'child runs ' \
    || CAST(COUNT(*) AS TEXT) FROM sqb_runs child JOIN sqb_runs parent ON parent.run_id = child.parent_run_id \
    UNION ALL SELECT 'documents ' || CAST(COUNT(*) AS TEXT) FROM sqb_snapshot_documents \
    JOIN sqb_documents USING (document_hash) UNION ALL SELECT 'runs ' || CAST(COUNT(*) AS TEXT) FROM sqb_runs";
const DANGLING_RUN_NODE: &str = "INSERT INTO sqb_run_nodes (run_id, node_key, status, writes) \
    VALUES ('018f0000-0000-7000-8000-0000000000ff', 'model.ghost', 'success', TRUE)";
const WIDEN_RUN_STATUS_POSTGRES: &str = "ALTER TABLE sqb_runs DROP CONSTRAINT sqb_runs_status_check; \
    ALTER TABLE sqb_runs ADD CONSTRAINT sqb_runs_status_check CHECK (status IN ('queued', \
    'launched', 'running', 'unresponsive', 'success', 'warn', 'partial', 'failed', 'cancelled', \
    'skipped'))";

#[test]
fn given_store_at_the_baseline_when_migrating_then_it_matches_a_new_store_and_keeps_its_rows() {
    let test_cases = [
        BaselineUpgradeTestCase {
            description: "SQLite store from a Linux or macOS release",
            dialect: &SQLITE_STORE,
            recorded_checksum: "228a951fe04d75eedcefdb44095afe652b178d9b2f32f3a5287c143877d39ce2",
            expected_version: current_version(MIGRATIONS),
        },
        BaselineUpgradeTestCase {
            description: "SQLite store from a Windows release, built from CRLF SQL",
            dialect: &SQLITE_STORE,
            recorded_checksum: "4cb5c0c4f392a2b3f4f2229e2ab8401870cd37b65346ada92cc70aa3ca2e0542",
            expected_version: current_version(MIGRATIONS),
        },
        BaselineUpgradeTestCase {
            description: "PostgreSQL store from a Linux or macOS release",
            dialect: &POSTGRES_STORE,
            recorded_checksum: "163045e32da43a8b0ddf17ea22f6dd41d1ff7bffe81836f3cbb35a8754915c3e",
            expected_version: current_version(MIGRATIONS),
        },
        BaselineUpgradeTestCase {
            description: "PostgreSQL store from a Windows release, built from CRLF SQL",
            dialect: &POSTGRES_STORE,
            recorded_checksum: "90758add355967a65ddfcf233b5e66893f92449e17c84e8f9bab42fb703917ab",
            expected_version: current_version(MIGRATIONS),
        },
    ];
    for test_case in test_cases {
        run(async {
            let (Some(upgraded), Some(created)) = (
                open_test_store(test_case.dialect.backend).await,
                open_test_store(test_case.dialect.backend).await,
            ) else {
                return;
            };
            create_unversioned_baseline(
                &upgraded.store,
                test_case.dialect,
                test_case.recorded_checksum,
            )
            .await;
            let before = table_rows(&upgraded.store, &POPULATED_TABLES).await;

            migrate_store(&upgraded.store)
                .await
                .expect("upgrade the baseline store");
            migrate_store(&created.store).await.expect("create a store");

            assert_eq!(
                (
                    schema_shape(&upgraded.store, test_case.dialect).await,
                    without_added_columns(
                        table_rows(&upgraded.store, &POPULATED_TABLES).await,
                        &before
                    ),
                    recorded_version(&upgraded.store).await,
                ),
                (
                    schema_shape(&created.store, test_case.dialect).await,
                    before,
                    Some(test_case.expected_version),
                ),
                "{}: a schema change needs a numbered migration that brings the baseline to it, \
                 and upgrading keeps every row",
                test_case.description
            );
            close_test_store(upgraded).await;
            close_test_store(created).await;
        });
    }
}

#[test]
fn given_a_store_from_before_service_accounts_when_upgraded_then_it_matches_a_new_store_and_takes_keys()
 {
    let test_cases = [
        ServiceAccountUpgradeTestCase {
            description: "SQLite store from the version before service accounts",
            dialect: &SQLITE_STORE,
            versions_behind: 0,
            expected_objects_before: 0,
            expected_version: current_version(MIGRATIONS),
        },
        ServiceAccountUpgradeTestCase {
            description: "PostgreSQL store from the version before service accounts",
            dialect: &POSTGRES_STORE,
            versions_behind: 0,
            expected_objects_before: 0,
            expected_version: current_version(MIGRATIONS),
        },
        ServiceAccountUpgradeTestCase {
            description: "SQLite store from two versions earlier",
            dialect: &SQLITE_STORE,
            versions_behind: 2,
            expected_objects_before: 0,
            expected_version: current_version(MIGRATIONS),
        },
        ServiceAccountUpgradeTestCase {
            description: "PostgreSQL store from two versions earlier",
            dialect: &POSTGRES_STORE,
            versions_behind: 2,
            expected_objects_before: 0,
            expected_version: current_version(MIGRATIONS),
        },
    ];
    for test_case in test_cases {
        run(async {
            let backend = test_case.dialect.backend;
            let upgraded = open_test_store(backend).await.expect("required backend");
            let created = open_test_store(backend).await.expect("required backend");
            let store = &upgraded.store;
            create_unversioned_baseline(
                store,
                test_case.dialect,
                test_case.dialect.baseline_checksum,
            )
            .await;
            let position = MIGRATIONS
                .iter()
                .position(|migration| migration.name == "service_accounts")
                .expect("the service accounts migration");
            migrate_with(store, &MIGRATIONS[..position - test_case.versions_behind])
                .await
                .expect("a store from an older release");
            let objects_before = schema_shape(store, test_case.dialect)
                .await
                .into_iter()
                .filter(|item| item.contains("service_account"))
                .count();
            let before = table_rows(store, &POPULATED_TABLES).await;

            migrate_store(store).await.expect("upgrade the older store");
            let kept = without_added_columns(table_rows(store, &POPULATED_TABLES).await, &before);
            let key = service_account_key(store).await;
            migrate_store(store)
                .await
                .expect("open the upgraded store again");
            migrate_store(&created.store).await.expect("create a store");
            let check = authenticate_service_key(store, &key, chrono::Utc::now())
                .await
                .expect("check the key");

            assert_eq!(
                (
                    objects_before,
                    schema_shape(store, test_case.dialect).await,
                    kept,
                    recorded_version(store).await,
                    matches!(check, ServiceKeyCheck::Accepted(_)),
                ),
                (
                    test_case.expected_objects_before,
                    schema_shape(&created.store, test_case.dialect).await,
                    before,
                    Some(test_case.expected_version),
                    true,
                ),
                "{}: the migration adds what a new store has, keeps every row and takes keys",
                test_case.description
            );
            close_test_store(upgraded).await;
            close_test_store(created).await;
        });
    }
}

#[test]
fn given_a_version_1_store_with_live_work_when_migrating_then_rows_default_and_references_join() {
    let test_cases = [
        LiveWorkUpgradeTestCase {
            description: "SQLite store",
            dialect: &SQLITE_STORE,
            foreign_key_check: "SELECT \"table\" || ' ' || parent AS item FROM pragma_foreign_key_check",
            expected_version: current_version(MIGRATIONS),
        },
        LiveWorkUpgradeTestCase {
            description: "PostgreSQL store",
            dialect: &POSTGRES_STORE,
            foreign_key_check: "SELECT 'enforced' AS item WHERE FALSE",
            expected_version: current_version(MIGRATIONS),
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.dialect.backend).await else {
                return;
            };
            let store = &fixture.store;
            create_unversioned_baseline(
                store,
                test_case.dialect,
                test_case.dialect.baseline_checksum,
            )
            .await;
            execute(store, VERSION_1_LIVE_WORK).await;

            migrate_store(store)
                .await
                .expect("upgrade the version 1 store");
            execute(store, SKIPPED_RUN).await;
            execute(store, VERSION_2_WRITES).await;

            assert_eq!(
                (
                    recorded_version(store).await,
                    query_items(store, test_case.foreign_key_check).await,
                    query_items(store, RUNS_AFTER_UPGRADE).await,
                    query_items(store, AUDITS_AFTER_UPGRADE).await,
                    query_items(store, JOINED_REFERENCES).await,
                ),
                (
                    Some(test_case.expected_version),
                    Vec::<String>::new(),
                    vec![
                        "018f0000-0000-7000-8000-000000000007 success 0".to_owned(),
                        "018f0000-0000-7000-8000-000000000008 skipped 0".to_owned(),
                        "018f0000-0000-7000-8000-000000000009 running 0".to_owned(),
                    ],
                    vec![
                        "orders_present counted".to_owned(),
                        "view rows 0".to_owned()
                    ],
                    vec![
                        "run nodes 1".to_owned(),
                        "slots 2".to_owned(),
                        "child runs 2".to_owned(),
                        "documents 2".to_owned(),
                        "runs 3".to_owned(),
                    ],
                ),
                "{}: version, broken references, runs, audit results and joined references",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_a_migration_rebuilding_a_referenced_table_when_migrating_then_rows_and_references_survive()
{
    let test_cases = [
        TableRebuildTestCase {
            description: "SQLite store",
            dialect: &SQLITE_STORE,
            expected_runs: 2,
            expected_version: 2,
        },
        TableRebuildTestCase {
            description: "PostgreSQL store",
            dialect: &POSTGRES_STORE,
            expected_runs: 2,
            expected_version: 2,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.dialect.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_with(store, &[]).await.expect("create the store");
            execute(store, BASELINE_ROWS).await;
            let mut before = table_rows(store, &POPULATED_TABLES).await;
            before.pop();
            let shape_before: Vec<String> = schema_shape(store, test_case.dialect)
                .await
                .into_iter()
                .filter(|item| !item.contains("'cancelled'"))
                .collect();
            let migration = StoreMigration {
                name: "widen_run_status",
                sqlite: rebuilt_runs_table(store).await,
                postgres: WIDEN_RUN_STATUS_POSTGRES,
                step: None,
            };

            migrate_with(store, &[migration])
                .await
                .expect("rebuild sqb_runs");
            execute(store, SKIPPED_RUN).await;

            let shape_after: Vec<String> = schema_shape(store, test_case.dialect)
                .await
                .into_iter()
                .filter(|item| !item.contains("'cancelled'"))
                .collect();
            let mut rows = table_rows(store, &POPULATED_TABLES).await;
            let runs = rows.pop().expect("sqb_runs rows");
            assert_eq!(
                (rows, runs.len(), shape_after, recorded_version(store).await),
                (
                    before,
                    test_case.expected_runs,
                    shape_before,
                    Some(test_case.expected_version)
                ),
                "{}: rows, other schema objects and version after the rebuild",
                test_case.description
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
            dialect: &SQLITE_STORE,
            tamper: "UPDATE sqb_schema_version SET version = 99",
            expected_fragments: vec!["schema is version 99, newer than this build's version"],
        },
        MigrationRefusalTestCase {
            description: "SQLite store older than the baseline",
            dialect: &SQLITE_STORE,
            tamper: "UPDATE sqb_schema_version SET checksum = 'unknown', version = NULL",
            expected_fragments: vec!["older than version 1", SQLITE_STORE.recreate_command],
        },
        MigrationRefusalTestCase {
            description: "SQLite store whose schema changed without a migration",
            dialect: &SQLITE_STORE,
            tamper: "UPDATE sqb_schema_version SET checksum = 'unknown'",
            expected_fragments: vec![
                "doesn't match this build's, although both are version",
                SQLITE_STORE.recreate_command,
            ],
        },
        MigrationRefusalTestCase {
            description: "PostgreSQL store from a newer build",
            dialect: &POSTGRES_STORE,
            tamper: "UPDATE sqb_schema_version SET version = 99",
            expected_fragments: vec!["schema is version 99, newer than this build's version"],
        },
        MigrationRefusalTestCase {
            description: "PostgreSQL store older than the baseline",
            dialect: &POSTGRES_STORE,
            tamper: "UPDATE sqb_schema_version SET checksum = 'unknown', version = NULL",
            expected_fragments: vec!["older than version 1", POSTGRES_STORE.recreate_command],
        },
        MigrationRefusalTestCase {
            description: "PostgreSQL store whose schema changed without a migration",
            dialect: &POSTGRES_STORE,
            tamper: "UPDATE sqb_schema_version SET checksum = 'unknown'",
            expected_fragments: vec![
                "doesn't match this build's, although both are version",
                POSTGRES_STORE.recreate_command,
            ],
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.dialect.backend).await else {
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

            let found: Vec<&str> = test_case
                .expected_fragments
                .iter()
                .copied()
                .filter(|fragment| error.contains(fragment))
                .collect();
            assert_eq!(
                (found, recorded_version(store).await),
                (test_case.expected_fragments.clone(), version_before),
                "{}: {error}",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_a_store_when_planning_its_migration_then_the_plan_is_reported_without_changing_it() {
    let current = current_version(MIGRATIONS);
    let expected_plans = || {
        (
            StoreMigrationPlan::Create { version: current },
            StoreMigrationPlan::Upgrade {
                from: 1,
                to: current,
                names: migration_names(MIGRATIONS, 1),
            },
            None,
        )
    };
    let expected_after = || {
        (
            StoreMigrationPlan::Current { version: current },
            StoreMigrationPlan::Upgrade {
                from: current,
                to: current + 1,
                names: vec![format!("{:04}_widen_run_status", current + 1)],
            },
        )
    };
    let test_cases = [
        StoreMigrationPlanTestCase {
            description: "SQLite store",
            dialect: &SQLITE_STORE,
            expected_plans: expected_plans(),
            expected_after: expected_after(),
        },
        StoreMigrationPlanTestCase {
            description: "PostgreSQL store",
            dialect: &POSTGRES_STORE,
            expected_plans: expected_plans(),
            expected_after: expected_after(),
        },
    ];
    for test_case in test_cases {
        run(async {
            let (Some(empty), Some(baseline)) = (
                open_test_store(test_case.dialect.backend).await,
                open_test_store(test_case.dialect.backend).await,
            ) else {
                return;
            };
            create_unversioned_baseline(
                &baseline.store,
                test_case.dialect,
                test_case.dialect.baseline_checksum,
            )
            .await;

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
            let mut with_pending: Vec<StoreMigration> = MIGRATIONS.to_vec();
            with_pending.push(StoreMigration {
                name: "widen_run_status",
                sqlite: "",
                postgres: "",
                step: None,
            });
            let after = (
                plan_store_migration(&baseline.store)
                    .await
                    .expect("plan the upgraded store"),
                migration_plan(
                    &baseline.store,
                    Some((schema_digest(&baseline.store).as_str(), Some(current))),
                    &with_pending,
                )
                .expect("plan a pending migration"),
            );

            assert_eq!(
                (plans, after),
                (test_case.expected_plans, test_case.expected_after),
                "{}",
                test_case.description
            );
            close_test_store(empty).await;
            close_test_store(baseline).await;
        });
    }
}

#[test]
fn given_this_builds_schema_when_checksummed_then_it_matches_the_checksum_pinned_for_its_version() {
    let test_cases = [
        PinnedChecksumTestCase {
            description: "SQLite store",
            dialect: &SQLITE_STORE,
            line_ending: "\n",
            expected_checksums: SQLITE_CHECKSUMS,
        },
        PinnedChecksumTestCase {
            description: "SQLite store built from a CRLF checkout",
            dialect: &SQLITE_STORE,
            line_ending: "\r\n",
            expected_checksums: SQLITE_CHECKSUMS,
        },
        PinnedChecksumTestCase {
            description: "PostgreSQL store",
            dialect: &POSTGRES_STORE,
            line_ending: "\n",
            expected_checksums: POSTGRES_CHECKSUMS,
        },
        PinnedChecksumTestCase {
            description: "PostgreSQL store built from a CRLF checkout",
            dialect: &POSTGRES_STORE,
            line_ending: "\r\n",
            expected_checksums: POSTGRES_CHECKSUMS,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.dialect.backend).await else {
                return;
            };
            let current = current_version(MIGRATIONS);
            let checked_out = |sql: &str| {
                sql.replace("\r\n", "\n")
                    .replace('\n', test_case.line_ending)
            };
            let checksum: String = schema_text_digest(
                &recall_schema(&fixture.store).join(";\n"),
                &checked_out(SCHEMA),
                &checked_out(INDEXES),
            );

            assert_eq!(
                Some(checksum.as_str()),
                test_case
                    .expected_checksums
                    .iter()
                    .find(|(version, _)| *version == current)
                    .map(|(_, pinned)| *pinned),
                "{}: schema.sql, indexes.sql or the Recall tables changed without a numbered \
                 migration; add one and pin the new version's checksum, never edit a pinned one",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_a_migration_that_breaks_a_reference_when_migrating_then_it_is_refused_unchanged() {
    let test_cases = [
        BrokenMigrationTestCase {
            description: "SQLite store",
            dialect: &SQLITE_STORE,
            expected_fragment: "foreign key references; the store is unchanged",
            expected_version: Some(1),
        },
        BrokenMigrationTestCase {
            description: "PostgreSQL store",
            dialect: &POSTGRES_STORE,
            expected_fragment: "violates foreign key constraint",
            expected_version: Some(1),
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.dialect.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_with(store, &[]).await.expect("create the store");
            let migration = StoreMigration {
                name: "dangling_run_node",
                sqlite: DANGLING_RUN_NODE,
                postgres: DANGLING_RUN_NODE,
                step: None,
            };

            let error = migrate_with(store, &[migration])
                .await
                .expect_err("refuse the migration")
                .to_string();

            assert_eq!(
                (
                    error.contains(test_case.expected_fragment),
                    recorded_version(store).await,
                    table_rows(store, &["sqb_run_nodes"]).await,
                ),
                (true, test_case.expected_version, vec![Vec::new()]),
                "{}: {error}",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
