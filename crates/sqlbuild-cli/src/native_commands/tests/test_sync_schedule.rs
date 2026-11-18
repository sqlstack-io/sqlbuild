use chrono::Utc;
use sqlbuild_orchestration::platform::models::WarehouseSyncFailure;
use sqlbuild_orchestration::warehouse_metadata::models::{MetadataBatch, WarehouseHistoryKind};
use uuid::Uuid;

use crate::native_commands::_helpers::location::execution::metadata_sync;
use crate::native_commands::_helpers::location::sync_report::completed_sync_next_at;
use crate::native_commands::models::LocationMetadataPage;
use crate::native_commands::tests::helpers::{acknowledge_complete_pages, one_location};
use crate::native_commands::tests::test_types::SyncCompletionTestCase;

#[test]
fn given_a_sync_page_report_when_it_completes_the_sync_then_it_names_the_next_safety_sync() {
    let last = WarehouseHistoryKind::ALL.len() - 1;
    let test_cases = [
        SyncCompletionTestCase {
            description: "the last page of the last table completes the sync",
            acknowledged_scans: last,
            more_pages: false,
            failure: None,
            safety_seconds: 1800,
            reads_wake_compute: false,
            expected_next_in_seconds: Some(1800),
        },
        SyncCompletionTestCase {
            description: "a page of an earlier table leaves the next sync unnamed",
            acknowledged_scans: 3,
            more_pages: false,
            failure: None,
            safety_seconds: 1800,
            reads_wake_compute: false,
            expected_next_in_seconds: None,
        },
        SyncCompletionTestCase {
            description: "the last table with more pages to come has not completed",
            acknowledged_scans: last,
            more_pages: true,
            failure: None,
            safety_seconds: 1800,
            reads_wake_compute: false,
            expected_next_in_seconds: None,
        },
        SyncCompletionTestCase {
            description: "a failed last page has not completed",
            acknowledged_scans: last,
            more_pages: false,
            failure: Some(WarehouseSyncFailure::Delivery),
            safety_seconds: 1800,
            reads_wake_compute: false,
            expected_next_in_seconds: None,
        },
        SyncCompletionTestCase {
            description: "with safety syncs off no next sync is due",
            acknowledged_scans: last,
            more_pages: false,
            failure: None,
            safety_seconds: 0,
            reads_wake_compute: false,
            expected_next_in_seconds: None,
        },
        SyncCompletionTestCase {
            description: "where reads wake compute no next sync is named, whatever the interval",
            acknowledged_scans: last,
            more_pages: false,
            failure: None,
            safety_seconds: 1800,
            reads_wake_compute: true,
            expected_next_in_seconds: None,
        },
    ];
    let now = Utc::now();
    for test_case in test_cases {
        let mut state = metadata_sync((one_location(), 1), (3600, test_case.safety_seconds));
        state.reads_wake_compute = test_case.reads_wake_compute;
        state.begin_sync(now);
        state = acknowledge_complete_pages(state, test_case.acknowledged_scans);
        state.pending = Some(LocationMetadataPage {
            batch: MetadataBatch {
                kind: state.kind(),
                session_id: Uuid::nil(),
                generation: 1,
                records: Vec::new(),
                scan: None,
                last_page: false,
            },
            next_source_after: test_case.more_pages.then(|| vec![Some("orders".into())]),
        });
        assert_eq!(
            completed_sync_next_at(&state, test_case.failure, now),
            test_case
                .expected_next_in_seconds
                .map(|seconds| now + chrono::Duration::seconds(seconds)),
            "{}",
            test_case.description
        );
    }
}
