use crate::helpers::{snapshot_contract, sqlite_failed_publish_and_schema_floor};
use crate::store_helpers::run;
use crate::store_test_types::Backend;
use crate::test_types::{RecoveryTestCase, SnapshotTestCase};

#[test]
fn given_control_store_when_snapshots_publish_then_generations_sources_and_changes_are_consistent()
{
    let test_cases = [
        SnapshotTestCase {
            description: "SQLite snapshot publication",
            backend: Backend::Sqlite,
            expected_generation: 6,
        },
        SnapshotTestCase {
            description: "PostgreSQL snapshot publication",
            backend: Backend::Postgres,
            expected_generation: 6,
        },
    ];
    for test_case in test_cases {
        let Some(observed) = run(snapshot_contract(test_case.backend)) else {
            continue;
        };
        assert_eq!(
            observed, test_case.expected_generation,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_sqlite_when_a_publish_fails_then_last_good_snapshot_survives_and_schema_drift_is_rejected()
{
    let test_cases = [RecoveryTestCase {
        description: "atomic rollback and recovery",
        expected_generation: 2,
    }];
    for test_case in test_cases {
        let observed = run(sqlite_failed_publish_and_schema_floor());
        assert_eq!(
            observed, test_case.expected_generation,
            "{}",
            test_case.description
        );
    }
}
