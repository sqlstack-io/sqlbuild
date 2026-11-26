//! A run-history sync is a problem while its newest report fails, since its first failure in a row; a success clears it.

use uuid::Uuid;

use crate::platform::_helpers::health::failing_syncs::failing_sync;
use crate::platform::models::{FailingSync, WarehouseReadFailure, WarehouseSyncFailure};
use crate::platform::tests::helpers::{failing_sync_of, sync};
use crate::platform::tests::test_types::FailingSyncTestCase;

#[test]
fn given_sync_reports_when_scanning_then_only_a_failing_newest_report_is_a_problem() {
    let unreached = Some(WarehouseReadFailure::Unreachable {});
    let read = Some(WarehouseSyncFailure::Read);
    let test_cases = [
        FailingSyncTestCase {
            description: "reads that never reach the warehouse, newest first, after a success",
            observations: vec![
                sync(3, read, (unreached, false)),
                sync(1, read, (unreached, false)),
                sync(-30, None, (None, false)),
            ],
            expected_sync: Some(failing_sync_of(false, true)),
        },
        FailingSyncTestCase {
            description: "failed reads stopped the sync, so it tries again only when resumed",
            observations: vec![
                sync(3, read, (Some(WarehouseReadFailure::ReadError {}), true)),
                sync(-30, None, (None, false)),
            ],
            expected_sync: Some(FailingSync {
                retry_at: None,
                ..failing_sync_of(true, false)
            }),
        },
        FailingSyncTestCase {
            description: "a newer success clears it",
            observations: vec![
                sync(5, None, (None, false)),
                sync(3, read, (unreached, false)),
            ],
            expected_sync: None,
        },
        FailingSyncTestCase {
            description: "waiting for a build that holds the warehouse file is no failure",
            observations: vec![sync(3, Some(WarehouseSyncFailure::Busy), (None, false))],
            expected_sync: None,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            failing_sync(&test_case.observations, (Uuid::nil(), "orders", "prod")),
            test_case.expected_sync,
            "{}",
            test_case.description
        );
    }
}
