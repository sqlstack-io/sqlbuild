use sqlbuild_orchestration::warehouse_metadata::constants::MAX_HISTORY_RECORDS;
use sqlbuild_orchestration::warehouse_metadata::models::{
    WarehouseHistoryFailure, WarehouseHistoryKind, WarehouseStateLocation,
};
use chrono::Utc;
use sqlbuild_config::project::models::ControlConfig;
use std::time::Duration;
use tokio::time::Instant;
use crate::native_commands::_helpers::execution::metadata_sync;
use crate::native_commands::tests::helpers::{acknowledge_complete_pages, one_location};
use crate::native_commands::tests::test_types::{
    MetadataBackoffTestCase, MetadataRetryTestCase, SafetyResumeTestCase, SafetySyncIdleTestCase,
    SafetySyncSettingTestCase, SyncCoalescingTestCase, UnchangedScanTestCase,
};
use sqlbuild_orchestration::warehouse_metadata::constants::{MAX_HISTORY_PAGE_RECORDS};
use crate::native_commands::_helpers::execution::{history_request, read_metadata_page, rows_text, safety_sync_setting, scan_line, sync_progress};
use crate::native_commands::constants::METADATA_SAFETY_SYNC_SECONDS;
use crate::native_commands::main::_location_metadata::read_failure_line;
use crate::native_commands::models::{LocationMetadataPage, LocationServe, ReadFailure};
use crate::native_commands::tests::test_types::{NewestVersionsTestCase, ReadFailureTestCase};

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
        let wait = state.fail_read(test_case.page_size, Utc::now());
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
            expected_limit: MAX_HISTORY_RECORDS,
            expected_halted: false,
            expected_paging: (true, true),
        },
        MetadataBackoffTestCase {
            description: "the fifth failure in a row stops the sync, which then never pages",
            failures: vec![],
            failures_after_request: vec![false, false, false, false, false],
            expected_waits: vec![Some(30), Some(60), Some(120), Some(240), None],
            expected_limit: MAX_HISTORY_RECORDS,
            expected_halted: true,
            expected_paging: (true, false),
        },
        MetadataBackoffTestCase {
            description: "a page-size error halves the page and retries after one poll",
            failures: vec![true, true],
            failures_after_request: vec![],
            expected_waits: vec![Some(30), Some(30)],
            expected_limit: MAX_HISTORY_RECORDS / 4,
            expected_halted: false,
            expected_paging: (false, false),
        },
        MetadataBackoffTestCase {
            description: "a sync request resumes a stopped sync with a fresh count",
            failures: vec![false, false, false, false, false],
            failures_after_request: vec![false],
            expected_waits: vec![Some(30), Some(60), Some(120), Some(240), None, Some(30)],
            expected_limit: MAX_HISTORY_RECORDS,
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
            .map(|page_size| state.fail_read(page_size, Utc::now()).map(|wait| wait.as_secs()))
            .collect();
        state.request_sync();
        if state.idle && state.sync_due(None) {
            state.begin_sync(Utc::now());
        }
        let paging_after_request = state.paging();
        waits.extend(
            test_case
                .failures_after_request
                .into_iter()
                .map(|page_size| state.fail_read(page_size, Utc::now()).map(|wait| wait.as_secs())),
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
fn given_a_stopped_sync_when_a_run_or_its_safety_sync_is_due_then_it_makes_one_attempt() {
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
            description: "a due safety sync resumes it, and one more failure stops it for another interval",
            safety_seconds: 1800,
            due_in: Duration::ZERO,
            billed: false,
            ran: false,
            expected_outcome: (true, true, None, true),
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
            description: "on a billed warehouse a run that ends after the stop resumes it",
            safety_seconds: 1800,
            due_in: Duration::from_secs(600),
            billed: true,
            ran: true,
            expected_outcome: (true, true, None, true),
        },
    ];
    for test_case in test_cases {
        let mut state = metadata_sync((one_location(), 1), (3600, test_case.safety_seconds));
        state.reads_wake_compute = test_case.billed;
        state.safety_explicit = !test_case.billed;
        state.begin_sync(Utc::now());
        let stopped: Vec<Option<Duration>> =
            (0..5).map(|_| state.fail_read(false, Utc::now())).collect();
        state.safety_at = Instant::now() + test_case.due_in;
        let runs_ended_at = test_case
            .ran
            .then(|| Utc::now() + chrono::Duration::seconds(1));
        let resumed: bool = state.idle && state.sync_due(runs_ended_at);
        if resumed {
            state.begin_sync(Utc::now());
        }
        let paging: bool = state.paging();
        let wait: Option<Duration> = state.fail_read(false, Utc::now());
        assert_eq!(
            (
                stopped[4],
                (resumed, paging, wait, state.halted),
                (state.safety_at.saturating_duration_since(Instant::now())
                    + Duration::from_secs(1))
                .as_secs()
                    >= test_case.safety_seconds,
            ),
            (None, test_case.expected_outcome, true),
            "{}",
            test_case.description
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
            server_limit: Some(5),
            expected: Some(5),
        },
        NewestVersionsTestCase {
            description: "a first read of check results stops at the limit",
            kind: WarehouseHistoryKind::AuditResults,
            incremental: false,
            server_limit: Some(20),
            expected: Some(20),
        },
        NewestVersionsTestCase {
            description: "an incremental read delivers every new build; pruning bounds the store",
            kind: WarehouseHistoryKind::Fingerprints,
            incremental: true,
            server_limit: Some(5),
            expected: None,
        },
        NewestVersionsTestCase {
            description: "other tables are never limited",
            kind: WarehouseHistoryKind::NodeResults,
            incremental: false,
            server_limit: Some(5),
            expected: None,
        },
        NewestVersionsTestCase {
            description: "a server without the setting gets the full history",
            kind: WarehouseHistoryKind::Fingerprints,
            incremental: false,
            server_limit: None,
            expected: None,
        },
    ];
    for test_case in test_cases {
        let mut state = metadata_sync((one_location(), 1), (3600, 0));
        state.index = WarehouseHistoryKind::ALL
            .iter()
            .position(|kind| *kind == test_case.kind)
            .expect("kind is listed");
        state.state_history_versions = test_case.server_limit;
        if test_case.incremental {
            state.observed_since = Some(Utc::now());
        }
        let request: serde_json::Value =
            serde_json::from_slice(&history_request(&state).expect("request")).expect("JSON");
        assert_eq!(
            request[test_case.kind.as_str()]
                .get("newest_versions")
                .and_then(serde_json::Value::as_u64),
            test_case.expected.map(u64::from),
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
            safety_sync_explicit: false,
            dev_events: false,
        };
        let mut state = metadata_sync((one_location(), 1), (3600, 0));
        state.index = WarehouseHistoryKind::ALL
            .iter()
            .position(|kind| *kind == test_case.kind)
            .expect("kind is listed");
        state.page_seconds = 1;
        let (state, page) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(read_metadata_page(
                (&options, &test_case.script),
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
    let mut state = metadata_sync((one_location(), 1), (3600, 0));
    state.index = 2;
    state.progress.scan_rows = 31_204;
    let line = scan_line(&state, 14);
    assert!(
        line.starts_with("Warehouse history 3/14: audit results, 31,204 rows in "),
        "{line}"
    );
    state.progress.scan_unchanged = true;
    assert!(scan_line(&state, 14).contains("audit results, unchanged in "));
    assert_eq!(
        [
            rows_text(0),
            rows_text(1),
            rows_text(999),
            rows_text(1_640),
            rows_text(1_234_567)
        ],
        [
            "0 rows",
            "1 row",
            "999 rows",
            "1,640 rows",
            "1,234,567 rows"
        ]
        .map(String::from)
    );
}

#[test]
fn given_a_delivered_page_when_reporting_health_then_progress_completes_only_on_the_last_kinds_last_page()
 {
    let page = |more: bool| LocationMetadataPage {
        batch: serde_json::from_value(serde_json::json!({
            "kind": "statistics",
            "session_id": uuid::Uuid::nil(),
            "generation": 1,
            "records": [{}, {}, {}],
        }))
        .expect("batch"),
        next_source_after: more.then(|| vec![Some("inventory".into())]),
    };
    let mut state = metadata_sync((one_location(), 1), (3600, 0));
    state.progress.sync_rows = 100;
    state.progress.scan_rows = 20;
    state.index = WarehouseHistoryKind::ALL.len() - 1;
    state.pending = Some(page(true));
    let progress = sync_progress(&state, true);
    assert_eq!(
        (
            progress.position,
            progress.kinds,
            progress.rows,
            progress.completed
        ),
        (14, 14, 123, false),
        "a page with more to come leaves the sync running"
    );
    state.pending = Some(page(false));
    assert!(
        sync_progress(&state, true).completed,
        "the last page completes the sync"
    );
    assert!(
        !sync_progress(&state, false).completed,
        "a failed report never completes it"
    );
    state.index = 2;
    assert!(
        !sync_progress(&state, true).completed,
        "an earlier kind's last page does not"
    );
}

#[test]
fn given_an_adapter_whose_reads_wake_compute_when_idle_then_only_runs_requests_or_a_set_interval_sync()
 {
    let idle = |wakes: bool, explicit: bool| {
        let mut state = metadata_sync((one_location(), 1), (3600, 1800));
        state.cycle_started_at = Some(Utc::now());
        state.safety_at = Instant::now() - Duration::from_secs(1);
        state.reads_wake_compute = wakes;
        state.safety_explicit = explicit;
        state
    };
    assert!(
        !idle(true, false).sync_due(None),
        "the default interval never wakes compute on a timer"
    );
    assert!(
        idle(false, false).sync_due(None),
        "metadata-only reads keep the default interval"
    );
    assert!(
        idle(true, true).sync_due(None),
        "an interval someone set still applies"
    );
    assert!(
        idle(true, false).sync_due(Some(Utc::now() + chrono::Duration::seconds(1))),
        "a run that ended still syncs"
    );
    let mut requested = idle(true, false);
    requested.request_sync();
    assert!(requested.sync_due(None), "a request still syncs");
}
