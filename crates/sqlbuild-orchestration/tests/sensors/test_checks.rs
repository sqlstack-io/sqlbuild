//! A check, audit or SQL test that fails or warns inside a run is an event, even when the run succeeds.

use std::collections::BTreeSet;

use serde_json::json;
use uuid::Uuid;

use crate::credentials::tests::location_helpers::create_location;
use crate::declarations::main::list_triggers::list_triggers;
use crate::runs::main::listings::list_runs::list_runs;
use crate::runs::models::{NodeOutcome, RunCommand, RunOutcome, RunStatus};
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::sensors::tests::helpers::{
    drain_sensor, finish_run_with_checks, record_as_adhoc, snapshot,
};
use crate::sensors::tests::test_types::CheckEventTestCase;
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::main::{
    begin::begin, commit::commit, migrate_store::migrate_store, query::query,
};
use crate::store::models::{Parameter, TransactionMode};
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_a_successful_run_with_a_failed_check_and_a_warned_audit_when_a_check_sensor_watches_them_then_each_matching_outcome_is_one_event()
 {
    let test_cases = [
        CheckEventTestCase {
            description: "SQLite failures and warnings",
            backend: Backend::Sqlite,
            kinds: &["failed", "warn"],
            adhoc_command: &[],
            expected_checks: &["audit:orders_amount", "check:orders_valid"],
        },
        CheckEventTestCase {
            description: "PostgreSQL failures only",
            backend: Backend::Postgres,
            kinds: &["failed"],
            adhoc_command: &[],
            expected_checks: &["check:orders_valid"],
        },
        CheckEventTestCase {
            description: "SQLite: a manual build is watched like a job run",
            backend: Backend::Sqlite,
            kinds: &["failed", "warn"],
            adhoc_command: &[RunCommand::Build],
            expected_checks: &["audit:orders_amount", "check:orders_valid"],
        },
        CheckEventTestCase {
            description: "SQLite: a standalone test run is not an event",
            backend: Backend::Sqlite,
            kinds: &["failed", "warn"],
            adhoc_command: &[RunCommand::Test],
            expected_checks: &[],
        },
        CheckEventTestCase {
            description: "SQLite: a standalone audit run is not an event",
            backend: Backend::Sqlite,
            kinds: &["failed", "warn"],
            adhoc_command: &[RunCommand::Audit],
            expected_checks: &[],
        },
        CheckEventTestCase {
            description: "PostgreSQL: a manual build is watched like a job run",
            backend: Backend::Postgres,
            kinds: &["failed", "warn"],
            adhoc_command: &[RunCommand::Build],
            expected_checks: &["audit:orders_amount", "check:orders_valid"],
        },
        CheckEventTestCase {
            description: "PostgreSQL: a standalone test run is not an event",
            backend: Backend::Postgres,
            kinds: &["failed", "warn"],
            adhoc_command: &[RunCommand::Test],
            expected_checks: &[],
        },
        CheckEventTestCase {
            description: "PostgreSQL: a standalone audit run is not an event",
            backend: Backend::Postgres,
            kinds: &["failed", "warn"],
            adhoc_command: &[RunCommand::Audit],
            expected_checks: &[],
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let location = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let mut input = snapshot(location);
            for (key, kind) in [
                ("check:orders_valid", "check"),
                ("audit:orders_amount", "audit"),
            ] {
                let mut node = input.nodes["orders"].clone();
                node.kind = kind.into();
                input.nodes.insert(key.into(), node);
            }
            input.documents.insert("jobs".into(), json!({
                "orders": {"description": "Refresh orders", "selection": ["orders", "customers"]},
                "inventory": {"description": "Recount inventory", "selection": ["orders"]}
            }));
            input.documents.insert("triggers".into(), json!({
                "orders_checks_sensor": {"description": "React to orders checks", "function": "start_inventory",
                    "code_hash": "v1", "timeout_seconds": 60, "minimum_interval_seconds": 1,
                    "when": {"kind": "check_event", "selection": ["check:orders_valid", "audit:orders_amount"],
                        "kinds": test_case.kinds}}
            }));
            publish_snapshot(store, &input).await.expect("publish");
            let trigger = list_triggers(store, location, "development")
                .await
                .expect("triggers")
                .remove(0);
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            let source = finish_run_with_checks(
                store,
                &lease,
                ((location, "development"), "orders"),
                RunOutcome::Success,
                &[
                    ("check:orders_valid", NodeOutcome::Failed),
                    ("audit:orders_amount", NodeOutcome::Warn),
                ],
            )
            .await;
            for command in test_case.adhoc_command {
                record_as_adhoc(store, source, *command).await;
            }
            assert_eq!(
                drain_sensor(store, &lease, trigger.trigger_id).await,
                test_case.expected_checks.len(),
                "{}: one run per matching check outcome",
                test_case.description
            );
            assert_eq!(
                drain_sensor(store, &lease, trigger.trigger_id).await,
                0,
                "{}: a delivered event is not delivered again",
                test_case.description
            );
            let (transaction, rows) = query(
                begin(store, TransactionMode::Read).await.expect("read"),
                "SELECT run_key FROM sqb_run_requests WHERE trigger_id=$1",
                vec![Parameter::Id(trigger.trigger_id)],
            )
            .await
            .expect("run keys");
            commit(transaction).await.expect("commit");
            let keys: BTreeSet<String> = rows
                .iter()
                .map(|row| row["run_key"].as_str().expect("run key").into())
                .collect();
            let expected: BTreeSet<String> = test_case
                .expected_checks
                .iter()
                .map(|check| format!("check:{source}:{check}:inventory"))
                .collect();
            assert_eq!(keys, expected, "{}", test_case.description);
            let runs = list_runs(store, (location, "development"), 100)
                .await
                .expect("runs");
            assert!(
                runs.iter()
                    .any(|run| run.run_id == source && run.status == RunStatus::Success),
                "{}: the watched run itself succeeded",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
