use sqlbuild_orchestration::warehouse_metadata::constants::MAX_HISTORY_RECORDS;
use sqlbuild_orchestration::warehouse_metadata::models::{
    WarehouseHistoryKind, WarehouseStateLocation,
};

use crate::native_commands::_helpers::execution::metadata_sync;
use crate::native_commands::tests::helpers::acknowledge_complete_pages;
use crate::native_commands::tests::test_types::MetadataRetryTestCase;

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
