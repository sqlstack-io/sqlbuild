use crate::native_commands::_helpers::command::paths::location_watch;
use crate::native_commands::_helpers::location::execution::metadata_sync;
use crate::native_commands::_helpers::location::execution::{
    finish_metadata_page, open_location_runtime,
};
use crate::native_commands::_helpers::location::execution::{
    history_request, read_metadata_page, safety_sync_setting,
};
use crate::native_commands::_helpers::location::sync_outcomes::failed_read;
use crate::native_commands::_helpers::location::sync_report::{
    metadata_health_of, read_failure_line, rows_text, scan_line, sync_progress,
};
use crate::native_commands::constants::{METADATA_FAILURE_LIMIT, METADATA_SAFETY_SYNC_SECONDS};
use crate::native_commands::main::_location_metadata::sync_location_metadata;
use crate::native_commands::models::{LocationMetadataPage, LocationServe, ReadFailure};
use crate::native_commands::tests::helpers::{
    acknowledge_complete_pages, location_options, one_location, published_location,
    republished_location,
};
use crate::native_commands::tests::test_types::BilledDeferralTestCase;
use crate::native_commands::tests::test_types::DynamicResumeTestCase;
use crate::native_commands::tests::test_types::FastClockScanTestCase;
use crate::native_commands::tests::test_types::RepublishTokenTestCase;
use crate::native_commands::tests::test_types::{
    FailedReadHealthTestCase, NewestVersionsTestCase, ReadFailureTestCase,
};
use crate::native_commands::tests::test_types::{
    MetadataBackoffTestCase, MetadataRetryTestCase, SafetyResumeTestCase, SafetySyncIdleTestCase,
    SafetySyncSettingTestCase, SkippedCountTestCase, StoppedPassTestCase, SyncCoalescingTestCase,
    UnchangedScanTestCase,
};
use crate::native_commands::tests::test_types::{
    RowsTextTestCase, ScanLineTestCase, SyncProgressTestCase, WakingIdleTestCase,
};
use chrono::{DateTime, Utc};
use sqlbuild_config::project::models::ControlConfig;
use sqlbuild_orchestration::platform::models::{
    LocationHealth, WarehouseReadFailure, WarehouseSyncFailure,
};
use sqlbuild_orchestration::warehouse_metadata::constants::MAX_HISTORY_PAGE_RECORDS;
use sqlbuild_orchestration::warehouse_metadata::constants::MAX_HISTORY_RECORDS;
use sqlbuild_orchestration::warehouse_metadata::models::{
    StatisticsRead, WarehouseHistoryFailure, WarehouseHistoryKind, WarehouseStateLocation,
};
use std::time::Duration;
use tokio::time::Instant;

#[test]
fn given_failed_metadata_read_when_back_at_statistics_then_page_shrinks_until_scan_completes() {
    let test_cases = [
        MetadataRetryTestCase {
            description: "a timed-out statistics page is retried smaller after catalog is revisited",
            page_size: true,
            failed_kind: WarehouseHistoryKind::Statistics,
            acknowledged_pages: 1,
            expected_statistics_limit: MAX_HISTORY_RECORDS / 2,
        },
        MetadataRetryTestCase {
            description: "a failed catalog page leaves the statistics page size alone",
            page_size: true,
            failed_kind: WarehouseHistoryKind::Catalog,
            acknowledged_pages: 1,
            expected_statistics_limit: MAX_HISTORY_RECORDS,
        },
        MetadataRetryTestCase {
            description: "a completed statistics scan restores the full statistics page",
            page_size: true,
            failed_kind: WarehouseHistoryKind::Statistics,
            acknowledged_pages: 1 + WarehouseHistoryKind::ALL.len(),
            expected_statistics_limit: MAX_HISTORY_RECORDS,
        },
        MetadataRetryTestCase {
            description: "a relation dropped while sampling revisits catalog without shrinking the page",
            page_size: false,
            failed_kind: WarehouseHistoryKind::Statistics,
            acknowledged_pages: 1,
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
        let wait = state.fail_read(test_case.page_size);
        assert!(wait.is_some(), "{}", test_case.description);
        let state = acknowledge_complete_pages(state, test_case.acknowledged_pages);
        assert_eq!(
            (state.kind(), state.statistics_limit, state.limit),
            (
                WarehouseHistoryKind::Statistics,
                test_case.expected_statistics_limit,
                MAX_HISTORY_PAGE_RECORDS,
            ),
            "{description}",
            description = test_case.description
        );
    }
}

#[test]
fn given_repeated_failed_reads_when_retrying_then_only_page_size_errors_shrink_and_failures_in_a_row_stop()
 {
    let test_cases = [
        MetadataBackoffTestCase {
            description: "a permission or missing-table error never shrinks the page, and a request ends its wait",
            failures: vec![false, false, false],
            failures_after_request: vec![],
            expected_waits: vec![Some(30), Some(60), Some(120)],
            expected_limit: MAX_HISTORY_PAGE_RECORDS,
            expected_halted: false,
            expected_paging: (true, true),
        },
        MetadataBackoffTestCase {
            description: "the fifth failure in a row stops the sync, which then never pages",
            failures: vec![],
            failures_after_request: vec![false, false, false, false, false],
            expected_waits: vec![Some(30), Some(60), Some(120), Some(240), None],
            expected_limit: MAX_HISTORY_PAGE_RECORDS,
            expected_halted: true,
            expected_paging: (true, false),
        },
        MetadataBackoffTestCase {
            description: "a page-size error halves the page and retries after one poll",
            failures: vec![true, true],
            failures_after_request: vec![],
            expected_waits: vec![Some(30), Some(30)],
            expected_limit: MAX_HISTORY_PAGE_RECORDS / 4,
            expected_halted: false,
            expected_paging: (false, false),
        },
        MetadataBackoffTestCase {
            description: "a sync request resumes a stopped sync with a fresh count",
            failures: vec![false, false, false, false, false],
            failures_after_request: vec![false],
            expected_waits: vec![Some(30), Some(60), Some(120), Some(240), None, Some(30)],
            expected_limit: MAX_HISTORY_PAGE_RECORDS,
            expected_halted: false,
            expected_paging: (true, false),
        },
    ];
    for test_case in test_cases {
        let mut state = metadata_sync((one_location(), 1), (3600, 0));
        state.index = WarehouseHistoryKind::ALL
            .iter()
            .position(|kind| *kind == WarehouseHistoryKind::Fingerprints)
            .expect("kind is listed");
        state.begin_sync(Utc::now());
        let mut waits: Vec<Option<u64>> = test_case
            .failures
            .into_iter()
            .map(|page_size| state.fail_read(page_size).map(|wait| wait.as_secs()))
            .collect();
        state.request_sync();
        (state.idle && state.sync_due(None)).then(|| state.begin_sync(Utc::now()));
        let paging_after_request = state.paging();
        waits.extend(
            test_case
                .failures_after_request
                .into_iter()
                .map(|page_size| state.fail_read(page_size).map(|wait| wait.as_secs())),
        );
        assert_eq!(
            (
                waits,
                state.limit,
                state.halted,
                (paging_after_request, state.paging())
            ),
            (
                test_case.expected_waits,
                test_case.expected_limit,
                test_case.expected_halted,
                test_case.expected_paging,
            ),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_stopped_sync_when_a_run_or_its_safety_sync_resumes_it_then_failed_reads_back_off_again()
{
    let test_cases = [
        SafetyResumeTestCase {
            description: "before the safety sync is due the sync stays stopped",
            safety_seconds: 1800,
            due_in: Duration::from_secs(600),
            billed: false,
            ran: false,
            expected_outcome: (false, false, None, true),
        },
        SafetyResumeTestCase {
            description: "a due safety sync resumes it with a fresh count, so one more failure backs off",
            safety_seconds: 1800,
            due_in: Duration::ZERO,
            billed: false,
            ran: false,
            expected_outcome: (true, true, Some(Duration::from_secs(30)), false),
        },
        SafetyResumeTestCase {
            description: "with safety syncs off only a run, a request or a publication resumes it",
            safety_seconds: 0,
            due_in: Duration::ZERO,
            billed: false,
            ran: false,
            expected_outcome: (false, false, None, true),
        },
        SafetyResumeTestCase {
            description: "on a billed warehouse a due default safety sync does not wake it",
            safety_seconds: 1800,
            due_in: Duration::ZERO,
            billed: true,
            ran: false,
            expected_outcome: (false, false, None, true),
        },
        SafetyResumeTestCase {
            description: "on a billed warehouse a run that ends after the stop resumes it with a fresh count",
            safety_seconds: 1800,
            due_in: Duration::from_secs(600),
            billed: true,
            ran: true,
            expected_outcome: (true, true, Some(Duration::from_secs(30)), false),
        },
    ];
    for test_case in test_cases {
        let mut state = metadata_sync((one_location(), 1), (3600, test_case.safety_seconds));
        state.reads_wake_compute = test_case.billed;
        state.begin_sync(Utc::now());
        let stopped: Vec<Option<Duration>> = (0..5).map(|_| state.fail_read(false)).collect();
        let stopped_for: Duration = state.safety_at.saturating_duration_since(Instant::now());
        state.safety_at = Instant::now() + test_case.due_in;
        let runs_ended_at = test_case
            .ran
            .then(|| Utc::now() + chrono::Duration::seconds(1));
        let resumed: bool = state.idle && state.sync_due(runs_ended_at);
        resumed.then(|| state.begin_sync(Utc::now()));
        let paging: bool = state.paging();
        let wait: Option<Duration> = state.fail_read(false);
        assert_eq!(
            (
                stopped[4],
                (resumed, paging, wait, state.halted),
                (stopped_for + Duration::from_secs(1)).as_secs() >= test_case.safety_seconds,
            ),
            (None, test_case.expected_outcome, true),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_sync_stopped_mid_pass_when_a_later_run_resumes_it_then_the_pass_restarts_and_still_replans()
 {
    let test_cases = [
        StoppedPassTestCase {
            description: "a run that ended before the stop by the server's clock does not resume it, \
                          though it ended after the stop by the location's clock",
            run_ended_after_stop: -1800,
            expected_outcome: (false, false, true),
        },
        StoppedPassTestCase {
            description: "a run that ended after the stop resumes it from the first table and the \
                          page it applied before stopping still replans",
            run_ended_after_stop: 60,
            expected_outcome: (true, true, true),
        },
    ];
    let statistics: usize = WarehouseHistoryKind::ALL
        .iter()
        .position(|kind| *kind == WarehouseHistoryKind::Statistics)
        .expect("statistics scan");
    for test_case in test_cases {
        let mut state = metadata_sync((one_location(), 1), (3600, 0));
        state.take_replan();
        let server_now = Utc::now() + chrono::Duration::hours(1);
        state.begin_sync(server_now);
        state.applied = true;
        state = acknowledge_complete_pages(state, statistics);
        (0..METADATA_FAILURE_LIMIT).for_each(|_| {
            let _ = state.fail_read(false);
        });
        let stopped_at = state.cycle_started_at.expect("stop time");
        let resumed: bool = state.sync_due(Some(
            server_now + chrono::Duration::seconds(test_case.run_ended_after_stop),
        ));
        resumed.then(|| state.begin_sync(Utc::now() + chrono::Duration::hours(1)));
        assert_eq!(
            (resumed, state.index == 0, state.take_replan()),
            test_case.expected_outcome,
            "{}",
            test_case.description
        );
        assert!(
            (stopped_at - server_now).num_seconds().abs() < 60,
            "the stop is timed by the server's clock"
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
fn given_a_table_skipped_as_unchanged_when_reporting_health_then_the_newest_mirrored_count_of_its_kind_repeats()
 {
    let test_cases = [
        SkippedCountTestCase {
            description: "one state location repeats its own read's count",
            locations: 1,
            later_read: None,
            expected_mirrored: Some(120),
        },
        SkippedCountTestCase {
            description: "a later read of the same kind in another location replaces the repeated count",
            locations: 2,
            later_read: Some(130),
            expected_mirrored: Some(130),
        },
    ];
    for test_case in test_cases {
        let locations: Vec<WarehouseStateLocation> = (0..test_case.locations)
            .map(|position| WarehouseStateLocation {
                database: Some("analytics".into()),
                schema: format!("state_{position}"),
            })
            .collect();
        let kinds = WarehouseHistoryKind::ALL.len();
        let skipped = (test_case.locations - 1) * kinds;
        let mut state = metadata_sync((locations, 1), (3600, 0));
        state.index = skipped;
        state.scan_token = Some("t1".into());
        state.scan_delivered = true;
        state.progress.scan_mirrored = Some(120);
        let state = acknowledge_complete_pages(state, 1);
        assert_eq!(
            state.progress.scan_mirrored, None,
            "{}: the next table starts without a count",
            test_case.description
        );
        let mut state = test_case
            .later_read
            .into_iter()
            .fold(state, |mut state, count| {
                state.index = 0;
                state.scan_token = Some("t2".into());
                state.scan_delivered = true;
                state.progress.scan_mirrored = Some(count);
                acknowledge_complete_pages(state, 1)
            });
        state.index = skipped;
        state.skip_unchanged();
        let health = metadata_health_of(&state, (Instant::now(), None, None), Utc::now());
        let LocationHealth::WarehouseSync {
            mirrored_records, ..
        } = health
        else {
            panic!("{}: not a warehouse sync report", test_case.description);
        };
        assert_eq!(
            mirrored_records, test_case.expected_mirrored,
            "{}",
            test_case.description
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
            safety_due_after_seconds: 3600,
            expected_follow_ups: 1,
        },
        SyncCoalescingTestCase {
            description: "a passed safety interval joins pending requests instead of adding a sync",
            requests: 2,
            run_ends_during_sync: false,
            safety_seconds: 1800,
            safety_due_after_seconds: 0,
            expected_follow_ups: 1,
        },
        SyncCoalescingTestCase {
            description: "a safety interval that passes during a sync adds no extra one",
            requests: 0,
            run_ends_during_sync: false,
            safety_seconds: 1800,
            safety_due_after_seconds: 0,
            expected_follow_ups: 0,
        },
        SyncCoalescingTestCase {
            description: "no signal leaves the location idle",
            requests: 0,
            run_ends_during_sync: false,
            safety_seconds: 1800,
            safety_due_after_seconds: 3600,
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
        state.safety_at = Instant::now() + Duration::from_secs(test_case.safety_due_after_seconds);
        state = acknowledge_complete_pages(state, WarehouseHistoryKind::ALL.len());
        assert!(
            state.idle,
            "{}: the first sync completed",
            test_case.description
        );
        let mut polled = Some(state);
        let follow_ups = (1..=5)
            .take_while(|poll| {
                let mut current = polled.take().expect("one state between polls");
                let due = current.sync_due(runs_ended_at);
                current.begin_sync(started + chrono::Duration::seconds(10 * poll));
                polled = Some(acknowledge_complete_pages(
                    current,
                    WarehouseHistoryKind::ALL.len(),
                ));
                due
            })
            .count();
        assert_eq!(
            follow_ups, test_case.expected_follow_ups,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_an_idle_location_when_the_safety_interval_passes_then_it_syncs_unless_turned_off() {
    let test_cases = [
        SafetySyncIdleTestCase {
            description: "the interval passed while idle",
            safety_seconds: 1800,
            safety_due_after_seconds: 0,
            expected_sync_due: true,
        },
        SafetySyncIdleTestCase {
            description: "the interval has not passed",
            safety_seconds: 1800,
            safety_due_after_seconds: 3600,
            expected_sync_due: false,
        },
        SafetySyncIdleTestCase {
            description: "0 turns safety syncs off",
            safety_seconds: 0,
            safety_due_after_seconds: 0,
            expected_sync_due: false,
        },
    ];
    let started = Utc::now();
    for test_case in test_cases {
        let mut state = metadata_sync((one_location(), 1), (3600, test_case.safety_seconds));
        state.begin_sync(started);
        state = acknowledge_complete_pages(state, WarehouseHistoryKind::ALL.len());
        state.safety_at = Instant::now() + Duration::from_secs(test_case.safety_due_after_seconds);
        assert_eq!(
            state.sync_due(None),
            test_case.expected_sync_due,
            "{}",
            test_case.description
        );
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
            expected_seconds: Ok(1800),
        },
        SafetySyncSettingTestCase {
            description: "the project file sets it",
            flag: None,
            environment: None,
            project: Some(3600),
            expected_seconds: Ok(3600),
        },
        SafetySyncSettingTestCase {
            description: "the environment overrides the project file",
            flag: None,
            environment: Some("2h"),
            project: Some(3600),
            expected_seconds: Ok(7200),
        },
        SafetySyncSettingTestCase {
            description: "a blank environment value falls through to the project file",
            flag: None,
            environment: Some(" "),
            project: Some(3600),
            expected_seconds: Ok(3600),
        },
        SafetySyncSettingTestCase {
            description: "the flag overrides the environment",
            flag: Some("off"),
            environment: Some("2h"),
            project: Some(3600),
            expected_seconds: Ok(0),
        },
        SafetySyncSettingTestCase {
            description: "a flag under a minute is refused",
            flag: Some("30s"),
            environment: None,
            project: None,
            expected_seconds: Err(
                "--sync-at-least-every must be a duration of at least '1m' such as '30m', or '0' or 'off' to turn safety syncs off",
            ),
        },
        SafetySyncSettingTestCase {
            description: "an unparseable environment value is refused",
            flag: None,
            environment: Some("-5m"),
            project: None,
            expected_seconds: Err(
                "SQLBUILD_CONTROL_SYNC_AT_LEAST_EVERY must be a duration of at least '1m' such as '30m', or '0' or 'off' to turn safety syncs off",
            ),
        },
    ];
    for test_case in test_cases {
        let resolved = safety_sync_setting(test_case.flag, test_case.environment, || {
            Ok(ControlConfig {
                sync_at_least_every_seconds: test_case.project,
                ..ControlConfig::default()
            })
        })
        .map(|seconds| seconds.unwrap_or(METADATA_SAFETY_SYNC_SECONDS));
        assert_eq!(
            resolved,
            test_case.expected_seconds.map_err(str::to_owned),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_state_history_limit_when_requesting_history_then_only_full_reads_of_builds_and_checks_carry_it()
 {
    let test_cases = [
        NewestVersionsTestCase {
            description: "a first read of builds stops at the limit",
            kind: WarehouseHistoryKind::Fingerprints,
            incremental: false,
            server_limit: 5,
            expected_newest_versions: Some(5),
        },
        NewestVersionsTestCase {
            description: "a first read of check results stops at the limit",
            kind: WarehouseHistoryKind::AuditResults,
            incremental: false,
            server_limit: 20,
            expected_newest_versions: Some(20),
        },
        NewestVersionsTestCase {
            description: "an incremental read delivers every new build; pruning bounds the store",
            kind: WarehouseHistoryKind::Fingerprints,
            incremental: true,
            server_limit: 5,
            expected_newest_versions: None,
        },
        NewestVersionsTestCase {
            description: "other tables are never limited",
            kind: WarehouseHistoryKind::NodeResults,
            incremental: false,
            server_limit: 5,
            expected_newest_versions: None,
        },
    ];
    for test_case in test_cases {
        let mut state = metadata_sync((one_location(), 1), (3600, 0));
        state.index = WarehouseHistoryKind::ALL
            .iter()
            .position(|kind| *kind == test_case.kind)
            .expect("kind is listed");
        state.state_history_versions = Some(test_case.server_limit);
        state.observed_since = test_case.incremental.then(Utc::now);
        let request: serde_json::Value =
            serde_json::from_slice(&history_request(&state).expect("request")).expect("JSON");
        assert_eq!(
            request[test_case.kind.as_str()]
                .get("newest_versions")
                .and_then(serde_json::Value::as_u64),
            test_case.expected_newest_versions.map(u64::from),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_failing_metadata_read_when_the_page_fails_then_the_cause_kind_and_page_size_are_reported()
 {
    let directory = tempfile::tempdir().expect("temporary directory");
    let reader = |name: &str, body: &str| {
        let path = directory.path().join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("reader script");
        let mut permissions = std::fs::metadata(&path).expect("metadata").permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
        std::fs::set_permissions(&path, permissions).expect("executable");
        path
    };
    let test_cases = [
        ReadFailureTestCase {
            description: "a reader that cannot start names the start failure",
            script: directory.path().join("missing-reader"),
            kind: WarehouseHistoryKind::Fingerprints,
            expected: |failure| matches!(failure, ReadFailure::ReaderStart(_)),
            expected_line: "Warehouse history read failed for fingerprints (2000-row page): the metadata reader did not start:",
        },
        ReadFailureTestCase {
            description: "a read error carries the warehouse's message",
            script: reader(
                "erroring-reader",
                r#"read line; echo '{"failed":{"reason":"permission denied for table _sqlbuild_audit_results","page_size":false}}'; read line"#,
            ),
            kind: WarehouseHistoryKind::AuditResults,
            expected: |failure| {
                *failure
                    == ReadFailure::Read(WarehouseHistoryFailure {
                        reason: "permission denied for table _sqlbuild_audit_results".into(),
                        page_size: false,
                    })
            },
            expected_line: "Warehouse history read failed for audit results (2000-row page): permission denied for table _sqlbuild_audit_results.",
        },
        ReadFailureTestCase {
            description: "a reader that never answers times out after the page allowance",
            script: reader("silent-reader", "read line; sleep 30"),
            kind: WarehouseHistoryKind::Catalog,
            expected: |failure| *failure == ReadFailure::TimedOut(1),
            expected_line: "Warehouse history read failed for catalog (2000-row page): no reply within 1 s.",
        },
    ];
    for test_case in test_cases {
        let options = LocationServe {
            project_dir: directory.path().to_path_buf(),
            server_url: "http://127.0.0.1:1".into(),
            location_id: uuid::Uuid::nil(),
            target: "development".into(),
            image_id: None,
            metadata_reconcile_seconds: 3600,
            safety_sync_seconds: 0,
            dev_events: false,
        };
        let mut state = metadata_sync((one_location(), 1), (3600, 0));
        state.index = WarehouseHistoryKind::ALL
            .iter()
            .position(|kind| *kind == test_case.kind)
            .expect("kind is listed");
        let (state, page) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(read_metadata_page(
                (&options, &test_case.script, 1),
                state,
                std::future::pending(),
            ))
            .expect("no control error")
            .expect("not stopped");
        let failure = page.expect_err(test_case.description);
        assert!(
            (test_case.expected)(&failure),
            "{}: {failure:?}",
            test_case.description
        );
        let line = read_failure_line(&state, &failure);
        assert!(
            line.starts_with(test_case.expected_line),
            "{}: {line}",
            test_case.description
        );
    }
}

#[test]
fn given_a_completed_table_when_reporting_progress_then_the_line_names_its_position_rows_and_time()
{
    let test_cases = [
        ScanLineTestCase {
            description: "a table that was read names its rows",
            index: 2,
            scan_rows: 31_204,
            unchanged: false,
            expected_prefix: "Warehouse history 3/14: audit results, 31,204 rows in ",
        },
        ScanLineTestCase {
            description: "a table found unchanged says so",
            index: 2,
            scan_rows: 0,
            unchanged: true,
            expected_prefix: "Warehouse history 3/14: audit results, unchanged in ",
        },
    ];
    for test_case in test_cases {
        let mut state = metadata_sync((one_location(), 1), (3600, 0));
        state.index = test_case.index;
        state.progress.scan_rows = test_case.scan_rows;
        state.progress.scan_unchanged = test_case.unchanged;
        let line: String = scan_line(&state, 14);
        assert!(
            line.starts_with(test_case.expected_prefix),
            "{}: {line}",
            test_case.description
        );
    }
}

#[test]
fn given_a_row_count_when_writing_progress_then_it_is_grouped_and_singular_for_one() {
    let test_cases = [
        RowsTextTestCase {
            description: "none",
            rows: 0,
            expected_text: "0 rows",
        },
        RowsTextTestCase {
            description: "one is singular",
            rows: 1,
            expected_text: "1 row",
        },
        RowsTextTestCase {
            description: "below a thousand",
            rows: 999,
            expected_text: "999 rows",
        },
        RowsTextTestCase {
            description: "thousands are grouped",
            rows: 1_640,
            expected_text: "1,640 rows",
        },
        RowsTextTestCase {
            description: "millions are grouped",
            rows: 1_234_567,
            expected_text: "1,234,567 rows",
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            rows_text(test_case.rows),
            test_case.expected_text,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_delivered_page_when_reporting_health_then_progress_completes_only_on_the_last_kinds_last_page()
 {
    let last: usize = WarehouseHistoryKind::ALL.len() - 1;
    let test_cases = [
        SyncProgressTestCase {
            description: "a page with more to come leaves the sync running",
            index: last,
            more_pages: true,
            clean: true,
            expected_progress: (14, 14, 123, false),
        },
        SyncProgressTestCase {
            description: "the last page completes the sync",
            index: last,
            more_pages: false,
            clean: true,
            expected_progress: (14, 14, 123, true),
        },
        SyncProgressTestCase {
            description: "a failed report never completes it",
            index: last,
            more_pages: false,
            clean: false,
            expected_progress: (14, 14, 123, false),
        },
        SyncProgressTestCase {
            description: "an earlier kind's last page does not",
            index: 2,
            more_pages: false,
            clean: true,
            expected_progress: (3, 14, 123, false),
        },
    ];
    for test_case in test_cases {
        let mut state = metadata_sync((one_location(), 1), (3600, 0));
        state.progress.sync_rows = 100;
        state.progress.scan_rows = 20;
        state.index = test_case.index;
        state.pending = Some(LocationMetadataPage {
            batch: serde_json::from_value(serde_json::json!({
                "kind": "statistics",
                "session_id": uuid::Uuid::nil(),
                "generation": 1,
                "records": [{}, {}, {}],
            }))
            .expect("batch"),
            next_source_after: test_case.more_pages.then(|| vec![Some("inventory".into())]),
        });
        let progress = sync_progress(&state, test_case.clean);
        assert_eq!(
            (
                progress.position,
                progress.kinds,
                progress.rows,
                progress.completed
            ),
            test_case.expected_progress,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_an_adapter_whose_reads_wake_compute_when_idle_then_only_runs_or_requests_sync() {
    let test_cases = [
        WakingIdleTestCase {
            description: "the interval, set or default, never wakes compute on a timer",
            wakes: true,
            run_ended: false,
            requested: false,
            expected_due: false,
        },
        WakingIdleTestCase {
            description: "metadata-only reads keep the default interval",
            wakes: false,
            run_ended: false,
            requested: false,
            expected_due: true,
        },
        WakingIdleTestCase {
            description: "a run that ended still syncs",
            wakes: true,
            run_ended: true,
            requested: false,
            expected_due: true,
        },
        WakingIdleTestCase {
            description: "a request still syncs",
            wakes: true,
            run_ended: false,
            requested: true,
            expected_due: true,
        },
    ];
    for test_case in test_cases {
        let mut state = metadata_sync((one_location(), 1), (3600, 1800));
        state.cycle_started_at = Some(Utc::now());
        state.safety_at = Instant::now() - Duration::from_secs(1);
        state.reads_wake_compute = test_case.wakes;
        test_case.requested.then(|| state.request_sync());
        let runs_ended_at = test_case
            .run_ended
            .then(|| Utc::now() + chrono::Duration::seconds(1));
        assert_eq!(
            state.sync_due(runs_ended_at),
            test_case.expected_due,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_failed_read_when_reporting_health_then_it_carries_cause_error_and_whether_it_stops() {
    let denied = || {
        ReadFailure::Read(WarehouseHistoryFailure {
            reason: "permission denied for table _sqlbuild_fingerprints".into(),
            page_size: false,
        })
    };
    let test_cases = [
        FailedReadHealthTestCase {
            description: "a time-out halves the page and never counts toward a stop",
            failure: ReadFailure::TimedOut(30),
            failures_before: 4,
            billed: false,
            expected_report: (
                true,
                Some(WarehouseReadFailure::TimedOut { seconds: 30 }),
                Some("no reply within 30 s"),
                false,
                None,
            ),
        },
        FailedReadHealthTestCase {
            description: "a read error before the limit backs off with its cause and error",
            failure: denied(),
            failures_before: 3,
            billed: false,
            expected_report: (
                false,
                Some(WarehouseReadFailure::ReadError {}),
                Some("permission denied for table _sqlbuild_fingerprints"),
                false,
                None,
            ),
        },
        FailedReadHealthTestCase {
            description: "the read error that stops the sync announces its safety retry",
            failure: denied(),
            failures_before: 4,
            billed: false,
            expected_report: (
                false,
                Some(WarehouseReadFailure::ReadError {}),
                Some("permission denied for table _sqlbuild_fingerprints"),
                true,
                Some(1800),
            ),
        },
        FailedReadHealthTestCase {
            description: "on a billed warehouse the stop waits for a run, request or publication",
            failure: denied(),
            failures_before: 4,
            billed: true,
            expected_report: (
                false,
                Some(WarehouseReadFailure::ReadError {}),
                Some("permission denied for table _sqlbuild_fingerprints"),
                true,
                None,
            ),
        },
        FailedReadHealthTestCase {
            description: "a reader that did not start reports a read error that names it",
            failure: ReadFailure::ReaderStart("no such file".into()),
            failures_before: 0,
            billed: false,
            expected_report: (
                false,
                Some(WarehouseReadFailure::ReadError {}),
                Some("the metadata reader did not start: no such file"),
                false,
                None,
            ),
        },
    ];
    for test_case in test_cases {
        let mut state = metadata_sync((one_location(), 1), (3600, 1800));
        state.index = WarehouseHistoryKind::ALL
            .iter()
            .position(|kind| *kind == WarehouseHistoryKind::Fingerprints)
            .expect("kind is listed");
        state.reads_wake_compute = test_case.billed;
        state.begin_sync(Utc::now());
        (0..test_case.failures_before).for_each(|_| {
            let _ = state.fail_read(false);
        });
        let page_size: bool = test_case.failure.page_size();
        let halts: bool = state.halts(page_size);
        let now = Utc::now();
        let health = metadata_health_of(
            &state,
            (
                Instant::now(),
                Some(WarehouseSyncFailure::Read),
                Some((&test_case.failure, halts)),
            ),
            now,
        );
        let LocationHealth::WarehouseSync {
            read_failure,
            error,
            progress,
            next_sync_at,
            ..
        } = health
        else {
            panic!("{}: not a warehouse sync report", test_case.description);
        };
        assert_eq!(
            (
                state.shrinks(page_size),
                read_failure,
                error.as_deref(),
                progress.expect("progress").halted,
                next_sync_at.map(|at| (at - now).num_seconds()),
            ),
            test_case.expected_report,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_published_location_when_its_sync_ends_on_the_local_statistics_page_then_billed_reads_keep_deferring()
 {
    let test_cases = [
        BilledDeferralTestCase {
            description: "a billed warehouse keeps deferring the safety sync",
            wakes_compute: true,
            expected_outcome: (false, true, false),
        },
        BilledDeferralTestCase {
            description: "metadata-only reads still run the safety sync",
            wakes_compute: false,
            expected_outcome: (false, false, true),
        },
    ];
    for test_case in test_cases {
        let directory = tempfile::tempdir().expect("temporary directory");
        let ((options, script), (client, poll), mut state) =
            published_location(directory.path(), test_case.wakes_compute);
        state.index = WarehouseHistoryKind::ALL
            .iter()
            .position(|kind| *kind == WarehouseHistoryKind::Statistics)
            .expect("kind is listed");
        state.watermark_loaded = true;
        state.statistics = Some(StatisticsRead {
            location: one_location().remove(0),
            names: Vec::new(),
            next_after: None,
        });
        let watch = location_watch(directory.path());
        let mut state = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(sync_location_metadata(
                state,
                (&options, &script, &watch),
                (&client, &poll),
                std::future::pending(),
            ))
            .expect("no control error")
            .expect("not stopped");
        state.cycle_started_at = Some(Utc::now());
        state.safety_at = Instant::now() - Duration::from_secs(1);
        assert_eq!(
            (
                state.halted,
                state.defers_safety_sync(),
                state.sync_due(None)
            ),
            test_case.expected_outcome,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_published_location_when_its_sync_stops_before_any_page_succeeds_then_billed_reads_keep_deferring()
 {
    let test_cases = [
        BilledDeferralTestCase {
            description: "a billed warehouse keeps deferring the safety sync",
            wakes_compute: true,
            expected_outcome: (true, true, false),
        },
        BilledDeferralTestCase {
            description: "metadata-only reads still run the safety sync",
            wakes_compute: false,
            expected_outcome: (true, false, true),
        },
    ];
    for test_case in test_cases {
        let directory = tempfile::tempdir().expect("temporary directory");
        let ((options, script), (client, poll), state) =
            published_location(directory.path(), test_case.wakes_compute);
        let mut state = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(async {
                let mut state = state;
                for _ in 0..METADATA_FAILURE_LIMIT {
                    let (read, page) =
                        read_metadata_page((&options, &script, 5), state, std::future::pending())
                            .await
                            .expect("no control error")
                            .expect("not stopped");
                    state = failed_read(
                        read,
                        (&client, &poll),
                        (Instant::now(), page.expect_err("every read fails")),
                        std::future::pending(),
                    )
                    .await
                    .expect("no control error")
                    .expect("not stopped");
                }
                state
            });
        state.cycle_started_at = Some(Utc::now());
        state.safety_at = Instant::now() - Duration::from_secs(1);
        assert_eq!(
            (
                state.halted,
                state.defers_safety_sync(),
                state.sync_due(None)
            ),
            test_case.expected_outcome,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_fast_location_clock_when_scanning_then_a_ddl_after_the_page_read_is_still_described_live()
 {
    let test_cases = [
        FastClockScanTestCase {
            description: "the first scan of a sync",
            scans_completed: 0,
            expected_described_live: true,
        },
        FastClockScanTestCase {
            description: "a later scan of the same pass",
            scans_completed: 1,
            expected_described_live: true,
        },
    ];
    // A synced plan reuses mirrored columns only when the last DDL ran this long before the scan began.
    let ddl_clock_skew = chrono::Duration::minutes(5);
    for test_case in test_cases {
        let mut state = metadata_sync((one_location(), 1), (3600, 0));
        state.begin_sync(Utc::now() - chrono::Duration::minutes(10));
        state = acknowledge_complete_pages(state, test_case.scans_completed);
        let page_read_at: DateTime<Utc> = Utc::now() + state.server_offset;
        let ddl_at: DateTime<Utc> = page_read_at + chrono::Duration::seconds(1);
        let (seconds, nanos) = state
            .scan_id
            .get_timestamp()
            .expect("a v7 scan id")
            .to_unix();
        let scan_started: DateTime<Utc> =
            DateTime::from_timestamp(i64::try_from(seconds).expect("seconds"), nanos)
                .expect("scan start");

        assert_eq!(
            (
                scan_started <= page_read_at,
                ddl_at > scan_started - ddl_clock_skew
            ),
            (true, test_case.expected_described_live),
            "{}: the scan started at {scan_started}, the page was read at {page_read_at}",
            test_case.description
        );
    }
}

#[test]
fn given_a_completed_sync_when_finishing_then_its_reader_resumes_expired_holds_once_before_closing()
{
    const UNTOKENED: &str = r#"{"dynamic_resume":[{"location":{"database":"analytics","schema":"state"},"quiet_token":null}]}"#;
    const TOKENED: &str = r#"{"dynamic_resume":[{"location":{"database":"analytics","schema":"state"},"quiet_token":"7/12:00"}]}"#;
    let test_cases = [
        DynamicResumeTestCase {
            description: "a completed sync sends the locations it read, then closes the reader",
            idle: true,
            halted: false,
            quiet_token: None,
            reply: r#"{"resumed":{"holds":[{"run_id":"run-1","relation":"analytics.marts.orders"}],"quiet_tokens":[null]}}"#,
            expected_requests: &[UNTOKENED],
            expected_reader_open: false,
            expected_quiet_token: None,
        },
        DynamicResumeTestCase {
            description: "a pass that leaves no hold open keeps its token for the next sync",
            idle: true,
            halted: false,
            quiet_token: None,
            reply: r#"{"resumed":{"holds":[],"quiet_tokens":["7/12:00"]}}"#,
            expected_requests: &[UNTOKENED],
            expected_reader_open: false,
            expected_quiet_token: Some("7/12:00"),
        },
        DynamicResumeTestCase {
            description: "a kept token is sent, and a pass that finds a hold drops it",
            idle: true,
            halted: false,
            quiet_token: Some("7/12:00"),
            reply: r#"{"resumed":{"holds":[],"quiet_tokens":[null]}}"#,
            expected_requests: &[TOKENED],
            expected_reader_open: false,
            expected_quiet_token: None,
        },
        DynamicResumeTestCase {
            description: "a failed resume only warns, keeps the token, and the reader still closes",
            idle: true,
            halted: false,
            quiet_token: Some("7/12:00"),
            reply: r#"{"failed":"permission denied"}"#,
            expected_requests: &[TOKENED],
            expected_reader_open: false,
            expected_quiet_token: Some("7/12:00"),
        },
        DynamicResumeTestCase {
            description: "an unreadable reply only warns, and the reader still closes",
            idle: true,
            halted: false,
            quiet_token: None,
            reply: "not json",
            expected_requests: &[UNTOKENED],
            expected_reader_open: false,
            expected_quiet_token: None,
        },
        DynamicResumeTestCase {
            description: "a sync stopped by failed reads writes nothing",
            idle: true,
            halted: true,
            quiet_token: None,
            reply: r#"{"resumed":{"holds":[],"quiet_tokens":["7/12:00"]}}"#,
            expected_requests: &[],
            expected_reader_open: false,
            expected_quiet_token: None,
        },
        DynamicResumeTestCase {
            description: "a sync with scans left neither resumes nor closes its reader",
            idle: false,
            halted: false,
            quiet_token: None,
            reply: r#"{"resumed":{"holds":[],"quiet_tokens":["7/12:00"]}}"#,
            expected_requests: &[],
            expected_reader_open: true,
            expected_quiet_token: None,
        },
    ];
    for test_case in test_cases {
        let directory = tempfile::tempdir().expect("temporary directory");
        let requests = directory.path().join("requests");
        let script = directory.path().join("location");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\nwhile read line; do echo \"$line\" >> '{}'; echo '{}'; done\n",
                requests.display(),
                test_case.reply
            ),
        )
        .expect("location script");
        let mut permissions = std::fs::metadata(&script).expect("metadata").permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
        std::fs::set_permissions(&script, permissions).expect("executable");
        let options = location_options(directory.path());
        let state = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(async {
                let mut state = metadata_sync((one_location(), 1), (3600, 1800));
                state.idle = test_case.idle;
                state.halted = test_case.halted;
                state.dynamic_quiet_tokens.extend(
                    test_case
                        .quiet_token
                        .map(|token| (one_location().remove(0), token.to_owned())),
                );
                state.reader =
                    Some(open_location_runtime(&options, &script, Vec::new()).expect("runtime"));
                finish_metadata_page(state, std::future::pending()).await
            })
            .expect("no control error")
            .expect("not stopped");
        let received = std::fs::read_to_string(&requests).unwrap_or_default();
        assert_eq!(
            (
                received.lines().collect::<Vec<_>>(),
                state.reader.is_some(),
                state
                    .dynamic_quiet_tokens
                    .get(&one_location().remove(0))
                    .map(String::as_str),
            ),
            (
                test_case.expected_requests.to_vec(),
                test_case.expected_reader_open,
                test_case.expected_quiet_token,
            ),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_quiet_pause_table_when_the_location_publishes_again_then_the_new_sync_keeps_its_token() {
    let test_cases = [
        RepublishTokenTestCase {
            description: "a publication after a quiet sync keeps the pause table's token",
            previous_token: Some("7/12:00"),
            expected_token: Some("7/12:00"),
        },
        RepublishTokenTestCase {
            description: "the first publication has no token yet",
            previous_token: None,
            expected_token: None,
        },
    ];
    for test_case in test_cases {
        let directory = tempfile::tempdir().expect("temporary directory");
        let mut previous = metadata_sync((one_location(), 1), (3600, 1800));
        previous.dynamic_quiet_tokens.extend(
            test_case
                .previous_token
                .map(|token| (one_location().remove(0), token.to_owned())),
        );
        let (_, _, state) = republished_location(directory.path(), false, Some(previous));
        assert_eq!(
            state
                .dynamic_quiet_tokens
                .get(&one_location().remove(0))
                .map(String::as_str),
            test_case.expected_token,
            "{}",
            test_case.description
        );
    }
}
