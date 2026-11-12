use sqlbuild_orchestration::warehouse_metadata::constants::MAX_HISTORY_RECORDS;
use sqlbuild_orchestration::warehouse_metadata::models::{
    WarehouseHistoryKind, WarehouseStateLocation,
};

use crate::native_commands::_helpers::execution::metadata_sync;
use crate::native_commands::tests::helpers::acknowledge_complete_pages;
use crate::native_commands::tests::test_types::{MetadataRetryTestCase, UnchangedScanTestCase};

#[test]
fn given_failed_metadata_read_when_back_at_statistics_then_page_shrinks_until_scan_completes() {
    let test_cases = [
        MetadataRetryTestCase {
            description: "a timed-out statistics page is retried smaller after catalog is revisited",
            failed_kind: WarehouseHistoryKind::Statistics,
            acknowledged_pages: 1,
            expected_statistics_limit: MAX_HISTORY_RECORDS / 2,
        },
        MetadataRetryTestCase {
            description: "a failed catalog page leaves the statistics page size alone",
            failed_kind: WarehouseHistoryKind::Catalog,
            acknowledged_pages: 1,
            expected_statistics_limit: MAX_HISTORY_RECORDS,
        },
        MetadataRetryTestCase {
            description: "a completed statistics scan restores the full statistics page",
            failed_kind: WarehouseHistoryKind::Statistics,
            acknowledged_pages: 1 + WarehouseHistoryKind::ALL.len(),
            expected_statistics_limit: MAX_HISTORY_RECORDS,
        },
    ];
    for test_case in test_cases {
        let location = WarehouseStateLocation {
            database: Some("analytics".into()),
            schema: "state".into(),
        };
        let mut state = metadata_sync((vec![location], 1), 3600);
        state.index = WarehouseHistoryKind::ALL
            .iter()
            .position(|kind| *kind == test_case.failed_kind)
            .expect("kind is listed");
        state.retry_read();
        let state = acknowledge_complete_pages(state, test_case.acknowledged_pages);
        assert_eq!(
            (state.kind(), state.statistics_limit, state.limit),
            (
                WarehouseHistoryKind::Statistics,
                test_case.expected_statistics_limit,
                MAX_HISTORY_RECORDS,
            ),
            "{description}",
            description = test_case.description
        );
    }
}

#[test]
fn given_recorded_change_token_when_scanning_again_then_only_proven_unchanged_reads_skip() {
    let test_cases = [
        UnchangedScanTestCase {
            description: "an incremental read lets the next incremental scan skip",
            read_reconciled: false,
            read_delivered: true,
            scan_reconciles: false,
            mirror_holds_rows: true,
            expected_token: Some("t1"),
        },
        UnchangedScanTestCase {
            description: "an incremental read never stands in for a reconciling scan",
            read_reconciled: false,
            read_delivered: true,
            scan_reconciles: true,
            mirror_holds_rows: true,
            expected_token: None,
        },
        UnchangedScanTestCase {
            description: "a reconciling read lets a reconciling scan skip",
            read_reconciled: true,
            read_delivered: true,
            scan_reconciles: true,
            mirror_holds_rows: true,
            expected_token: Some("t1"),
        },
        UnchangedScanTestCase {
            description: "a mirror that lost delivered rows is read again",
            read_reconciled: true,
            read_delivered: true,
            scan_reconciles: false,
            mirror_holds_rows: false,
            expected_token: None,
        },
        UnchangedScanTestCase {
            description: "an empty table with an empty mirror stays skipped",
            read_reconciled: true,
            read_delivered: false,
            scan_reconciles: true,
            mirror_holds_rows: false,
            expected_token: Some("t1"),
        },
    ];
    for test_case in test_cases {
        let location = WarehouseStateLocation {
            database: Some("analytics".into()),
            schema: "state".into(),
        };
        let mut state = metadata_sync((vec![location], 1), 3600);
        state.reconcile = test_case.read_reconciled;
        state.scan_token = Some("t1".into());
        state.scan_delivered = test_case.read_delivered;
        let mut state = acknowledge_complete_pages(state, 1);
        assert_eq!(state.kind(), WarehouseHistoryKind::SourceFreshness);
        state.index = 0;
        state.reconcile = test_case.scan_reconciles;
        state.mirror_holds_rows = test_case.mirror_holds_rows;
        assert_eq!(
            state.unchanged_token(),
            test_case.expected_token,
            "{description}",
            description = test_case.description
        );
    }
}

#[test]
fn given_recorded_change_token_when_a_later_read_has_none_then_the_scan_reads_again() {
    let location = WarehouseStateLocation {
        database: Some("analytics".into()),
        schema: "state".into(),
    };
    let mut state = metadata_sync((vec![location], 1), 3600);
    state.scan_token = Some("t1".into());
    let mut state = acknowledge_complete_pages(state, 1);
    state.index = 0;
    let mut state = acknowledge_complete_pages(state, 1);
    state.index = 0;
    state.mirror_holds_rows = true;
    assert_eq!(state.unchanged_token(), None);
}
