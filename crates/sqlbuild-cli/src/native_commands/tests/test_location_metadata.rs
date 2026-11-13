use sqlbuild_orchestration::warehouse_metadata::constants::MAX_HISTORY_RECORDS;
use sqlbuild_orchestration::warehouse_metadata::models::{
    WarehouseHistoryKind, WarehouseStateLocation,
};

use chrono::Utc;
use sqlbuild_config::project::models::ControlConfig;
use tokio::time::Instant;

use crate::native_commands::_helpers::execution::{metadata_sync, safety_sync_seconds};
use crate::native_commands::tests::helpers::{acknowledge_complete_pages, one_location};
use crate::native_commands::tests::test_types::{
    MetadataRetryTestCase, SafetySyncSettingTestCase, SyncCoalescingTestCase, UnchangedScanTestCase,
};

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
        let mut state = metadata_sync((vec![location], 1), (3600, 0));
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
            read_incremental: false,
            rereads_without_token: 0,
            scan_reconciles: false,
            mirror_holds_rows: true,
            expected_token: Some("t1"),
        },
        UnchangedScanTestCase {
            description: "an incremental read never stands in for a reconciling scan",
            read_reconciled: false,
            read_delivered: true,
            read_incremental: false,
            rereads_without_token: 0,
            scan_reconciles: true,
            mirror_holds_rows: true,
            expected_token: None,
        },
        UnchangedScanTestCase {
            description: "a reconciling read lets a reconciling scan skip",
            read_reconciled: true,
            read_delivered: true,
            read_incremental: false,
            rereads_without_token: 0,
            scan_reconciles: true,
            mirror_holds_rows: true,
            expected_token: Some("t1"),
        },
        UnchangedScanTestCase {
            description: "a mirror that lost delivered rows is read again",
            read_reconciled: true,
            read_delivered: true,
            read_incremental: false,
            rereads_without_token: 0,
            scan_reconciles: false,
            mirror_holds_rows: false,
            expected_token: None,
        },
        UnchangedScanTestCase {
            description: "an empty table with an empty mirror stays skipped",
            read_reconciled: true,
            read_delivered: false,
            read_incremental: false,
            rereads_without_token: 0,
            scan_reconciles: true,
            mirror_holds_rows: false,
            expected_token: Some("t1"),
        },
        UnchangedScanTestCase {
            description: "an incremental read that delivered nothing still guards a lost mirror",
            read_reconciled: false,
            read_delivered: false,
            read_incremental: true,
            rereads_without_token: 0,
            scan_reconciles: false,
            mirror_holds_rows: false,
            expected_token: None,
        },
        UnchangedScanTestCase {
            description: "a later read without a token forgets the recorded one",
            read_reconciled: false,
            read_delivered: true,
            read_incremental: false,
            rereads_without_token: 1,
            scan_reconciles: false,
            mirror_holds_rows: true,
            expected_token: None,
        },
    ];
    for test_case in test_cases {
        let location = WarehouseStateLocation {
            database: Some("analytics".into()),
            schema: "state".into(),
        };
        let mut state = metadata_sync((vec![location], 1), (3600, 0));
        state.reconcile = test_case.read_reconciled;
        state.scan_token = Some("t1".into());
        state.scan_delivered = test_case.read_delivered;
        state.observed_since = test_case.read_incremental.then(chrono::Utc::now);
        let mut state = acknowledge_complete_pages(state, 1);
        assert_eq!(
            state.kind(),
            WarehouseHistoryKind::SourceFreshness,
            "{description}",
            description = test_case.description
        );
        state.index = 0;
        let mut state = (0..test_case.rereads_without_token).fold(state, |state, _| {
            let mut state = acknowledge_complete_pages(state, 1);
            state.index = 0;
            state
        });
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
fn given_signals_during_a_sync_when_it_completes_then_exactly_one_follow_up_sync_runs() {
    let test_cases = [
        SyncCoalescingTestCase {
            description: "four build requests and a run ending share one follow-up",
            requests: 4,
            run_ends_during_sync: true,
            safety_seconds: 1800,
            safety_passes_during_sync: false,
            expected_follow_ups: 1,
        },
        SyncCoalescingTestCase {
            description: "a passed safety interval joins pending requests instead of adding a sync",
            requests: 2,
            run_ends_during_sync: false,
            safety_seconds: 1800,
            safety_passes_during_sync: true,
            expected_follow_ups: 1,
        },
        SyncCoalescingTestCase {
            description: "a safety interval that passes during a sync adds no extra one",
            requests: 0,
            run_ends_during_sync: false,
            safety_seconds: 1800,
            safety_passes_during_sync: true,
            expected_follow_ups: 0,
        },
        SyncCoalescingTestCase {
            description: "no signal leaves the location idle",
            requests: 0,
            run_ends_during_sync: false,
            safety_seconds: 1800,
            safety_passes_during_sync: false,
            expected_follow_ups: 0,
        },
    ];
    for test_case in test_cases {
        let started = Utc::now();
        let mut state = metadata_sync((one_location(), 1), (3600, test_case.safety_seconds));
        assert!(
            state.sync_due(None),
            "{}: the first sync is due",
            test_case.description
        );
        state.begin_sync(started);
        for _ in 0..test_case.requests {
            state.request_sync();
        }
        let runs_ended_at = test_case
            .run_ends_during_sync
            .then(|| started + chrono::Duration::seconds(1));
        if test_case.safety_passes_during_sync {
            state.safety_at = Instant::now();
        }
        state = acknowledge_complete_pages(state, WarehouseHistoryKind::ALL.len());
        assert!(
            state.idle,
            "{}: the first sync completed",
            test_case.description
        );
        let mut follow_ups = 0;
        for poll in 1..=5 {
            if state.sync_due(runs_ended_at) {
                follow_ups += 1;
                state.begin_sync(started + chrono::Duration::seconds(10 * poll));
                state = acknowledge_complete_pages(state, WarehouseHistoryKind::ALL.len());
            }
        }
        assert_eq!(
            follow_ups, test_case.expected_follow_ups,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_an_idle_location_when_the_safety_interval_passes_then_it_syncs_unless_turned_off() {
    let started = Utc::now();
    for (description, safety_seconds, passed, expected) in [
        ("the interval passed while idle", 1800, true, true),
        ("the interval has not passed", 1800, false, false),
        ("0 turns safety syncs off", 0, true, false),
    ] {
        let mut state = metadata_sync((one_location(), 1), (3600, safety_seconds));
        state.begin_sync(started);
        state = acknowledge_complete_pages(state, WarehouseHistoryKind::ALL.len());
        if passed {
            state.safety_at = Instant::now();
        }
        assert_eq!(state.sync_due(None), expected, "{description}");
    }
}

#[test]
fn given_safety_sync_settings_when_resolving_then_flag_beats_environment_beats_project() {
    let test_cases = [
        SafetySyncSettingTestCase {
            description: "nothing set uses thirty minutes",
            flag: None,
            environment: None,
            project: None,
            expected: Ok(1800),
        },
        SafetySyncSettingTestCase {
            description: "the project file sets it",
            flag: None,
            environment: None,
            project: Some(3600),
            expected: Ok(3600),
        },
        SafetySyncSettingTestCase {
            description: "the environment overrides the project file",
            flag: None,
            environment: Some("2h"),
            project: Some(3600),
            expected: Ok(7200),
        },
        SafetySyncSettingTestCase {
            description: "a blank environment value falls through to the project file",
            flag: None,
            environment: Some(" "),
            project: Some(3600),
            expected: Ok(3600),
        },
        SafetySyncSettingTestCase {
            description: "the flag overrides the environment",
            flag: Some("off"),
            environment: Some("2h"),
            project: Some(3600),
            expected: Ok(0),
        },
        SafetySyncSettingTestCase {
            description: "a flag under a minute is refused",
            flag: Some("30s"),
            environment: None,
            project: None,
            expected: Err("--sync-at-least-every must be a duration of at least '1m'"),
        },
        SafetySyncSettingTestCase {
            description: "an unparseable environment value is refused",
            flag: None,
            environment: Some("-5m"),
            project: None,
            expected: Err(
                "SQLBUILD_CONTROL_SYNC_AT_LEAST_EVERY must be a duration of at least '1m'",
            ),
        },
    ];
    for test_case in test_cases {
        let resolved = safety_sync_seconds(test_case.flag, test_case.environment, || {
            Ok(ControlConfig {
                sync_at_least_every_seconds: test_case.project,
                ..ControlConfig::default()
            })
        });
        match (resolved, test_case.expected) {
            (Ok(seconds), Ok(expected)) => {
                assert_eq!(seconds, expected, "{}", test_case.description);
            }
            (Err(message), Err(fragment)) => assert!(
                message.starts_with(fragment),
                "{}: {message}",
                test_case.description
            ),
            (resolved, expected) => panic!(
                "{}: resolved {resolved:?}, expected {expected:?}",
                test_case.description
            ),
        }
    }
}
