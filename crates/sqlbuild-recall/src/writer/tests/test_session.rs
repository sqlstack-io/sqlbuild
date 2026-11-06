use std::time::Duration;

use crate::records::models::{OutputStream, RecallValue};
use crate::schema::models::Table;
use crate::store::errors::StoreErrorKind;
use crate::writer::models::{Priority, SetupOutcome};

#[cfg(unix)]
use crate::writer::tests::helpers::unwritable_session;
use crate::writer::tests::helpers::{
    artifact_counts, close_under_held_lock, close_with_slow_final_rows, creation_notices,
    event_counts, insert_heartbeat, invocation_during_discovery, output_stored_while_running,
    partially_inserted, pending_event_counts, record_run, recorded_session, replaced_session,
    replacement_order, sent_before_the_window, session_on_corrupt_store,
    written_to_core_release_store,
};
#[cfg(unix)]
use crate::writer::tests::test_types::UnwritableTestCase;
use crate::writer::tests::test_types::{
    ArtifactSizeTestCase, CoreReleaseStoreTestCase, CorruptStoreTestCase, CreationNoticeTestCase,
    DiscoveryTestCase, EarlyBatchTestCase, EventBuildTestCase, HeldLockTestCase,
    PartialInsertTestCase, PendingEventTestCase, RecordedSession, ReplacementOrderTestCase,
    ReplacementTestCase, SessionTestCase, SlowFinalRowsTestCase, StoredWhileRunningTestCase,
};

const CLOSE_SLACK_SECONDS: f64 = 0.3;

#[test]
fn given_a_session_when_closed_then_every_record_and_the_delivery_rows_are_written() {
    let large: Vec<u8> = (0..200_000_u32)
        .map(|value| u8::try_from(value % 251).unwrap_or_default())
        .collect();
    let test_cases = [SessionTestCase {
        description: "binary output, events, filtered diagnostics and deduplicated SQL land",
        output: vec![
            (OutputStream::Stdout, b"start\n".to_vec()),
            (OutputStream::Stderr, b"\xff\x00warn\n".to_vec()),
            (OutputStream::Stdout, large.clone()),
        ],
        expected_session: RecordedSession {
            setup: Some(SetupOutcome::Ready),
            lost: (0, 0),
            output_fully_delivered: true,
            transcript: (
                [b"start\n".as_slice(), b"\xff\x00warn\n", &large].concat(),
                true,
            ),
            stdout: [b"start\n".as_slice(), &large].concat(),
            facts: vec![vec![
                RecallValue::Text("completed".to_owned()),
                RecallValue::Integer(1),
                RecallValue::Integer(1),
                RecallValue::Integer(5),
                RecallValue::Text("build".to_owned()),
            ]],
        },
    }];

    for test_case in test_cases {
        assert_eq!(
            recorded_session(&test_case.output),
            test_case.expected_session,
            "{}",
            test_case.description
        );
    }
}

#[cfg(unix)]
#[test]
fn given_an_unwritable_store_when_started_then_setup_reports_permission_and_close_is_quick() {
    let test_cases = [UnwritableTestCase {
        description: "permission is reported at setup and the summary is degraded",
        expected_permission_and_degraded: (true, true),
    }];

    for test_case in test_cases {
        assert_eq!(
            unwritable_session(),
            test_case.expected_permission_and_degraded,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_held_write_lock_when_closed_then_the_shutdown_budget_caps_the_delay() {
    let test_cases = [HeldLockTestCase {
        description: "a locked store costs at most the shutdown budget at exit",
        shutdown_ms: 700,
        expected_degraded: true,
    }];

    for test_case in test_cases {
        let (elapsed, degraded) =
            close_under_held_lock(Duration::from_millis(test_case.shutdown_ms));
        let budget: f64 = Duration::from_millis(test_case.shutdown_ms).as_secs_f64();
        assert!(
            elapsed <= budget + CLOSE_SLACK_SECONDS,
            "{}: {elapsed:.3}s",
            test_case.description
        );
        assert_eq!(
            degraded, test_case.expected_degraded,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_slow_final_delivery_rows_when_closed_then_the_shutdown_budget_bounds_them() {
    let test_cases = [
        SlowFinalRowsTestCase {
            description: "final rows slower than half a second but inside the budget record complete",
            shutdown: Duration::from_secs(5),
            final_rows_delay: Duration::from_millis(1_500),
            expected_flush_complete: true,
        },
        SlowFinalRowsTestCase {
            description: "final rows slower than the shutdown budget still exit on time",
            shutdown: Duration::from_millis(1_000),
            final_rows_delay: Duration::from_secs(10),
            expected_flush_complete: false,
        },
    ];

    for test_case in test_cases {
        let (elapsed, flush_complete) =
            close_with_slow_final_rows(test_case.shutdown, test_case.final_rows_delay);
        assert!(
            elapsed <= test_case.shutdown.as_secs_f64() + CLOSE_SLACK_SECONDS,
            "{}: {elapsed:.3}s",
            test_case.description
        );
        assert_eq!(
            flush_complete, test_case.expected_flush_complete,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_file_that_is_not_a_database_when_started_then_it_is_moved_aside_once() {
    let test_cases = [CorruptStoreTestCase {
        description: "the corrupt file is kept aside and a fresh store records the run",
        expected_notices: 1,
        expected_moved: true,
        expected_rows: vec![vec![RecallValue::Text("unknown".to_owned())]],
    }];

    for test_case in test_cases {
        assert_eq!(
            session_on_corrupt_store(),
            (
                test_case.expected_notices,
                test_case.expected_moved,
                test_case.expected_rows
            ),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_sql_of_different_sizes_when_published_then_oversized_sql_is_dropped_unhashed() {
    let test_cases = [
        ArtifactSizeTestCase {
            description: "ordinary SQL is delivered with its reference",
            length: 64,
            times: 1,
            expected_counts: (1, 1, 0, vec![vec![RecallValue::Integer(1)]]),
        },
        ArtifactSizeTestCase {
            description: "SQL seen twice is stored once and both records count as delivered",
            length: 64,
            times: 2,
            expected_counts: (2, 2, 0, vec![vec![RecallValue::Integer(1)]]),
        },
        ArtifactSizeTestCase {
            description: "SQL above the artifact limit is counted as dropped",
            length: 4 * 1024 * 1024 + 1,
            times: 1,
            expected_counts: (1, 0, 1, vec![vec![RecallValue::Integer(0)]]),
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            artifact_counts(test_case.length, test_case.times),
            test_case.expected_counts,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_events_when_built_on_the_delivery_thread_then_unbuildable_ones_count_as_failed() {
    let valid: &str = r#"{"event_id":"e1","event_type":"invocation_started","schema_version":2,
        "producer":"sqlbuild","producer_version":"1","occurred_at":"2026-10-19T00:00:00.000000Z",
        "invocation_id":"events","invocation_sequence":1,"external_context":{},"payload":{}}"#;
    let test_cases = [
        EventBuildTestCase {
            description: "a complete event is delivered",
            events: vec![valid],
            expected_counts: (1, 1, 0),
        },
        EventBuildTestCase {
            description: "an event without an ID is accepted, then counted as failed",
            events: vec![valid, r#"{"event_type":"invocation_started"}"#],
            expected_counts: (2, 1, 1),
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            event_counts(&test_case.events),
            test_case.expected_counts,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_heartbeats_due_faster_than_batches_fill_when_running_then_queued_records_are_still_written()
 {
    let test_cases = [StoredWhileRunningTestCase {
        description: "a 50 ms heartbeat still lets queued output reach the store",
        heartbeat: Duration::from_millis(50),
        wait: Duration::from_millis(1_500),
        expected_rows: vec![vec![RecallValue::Integer(1)]],
    }];

    for test_case in test_cases {
        assert_eq!(
            output_stored_while_running(test_case.heartbeat, test_case.wait),
            test_case.expected_rows,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_an_output_replacement_when_closed_then_published_filtered_and_failed_chunks_are_accounted()
{
    let accounting = vec![vec![
        RecallValue::Integer(1),
        RecallValue::Integer(1),
        RecallValue::Integer(2),
        RecallValue::Integer(1),
        RecallValue::Text("dropped".to_owned()),
    ]];
    let output: &[(OutputStream, &[u8])] = &[
        (OutputStream::Stderr, b"drop that\n"),
        (OutputStream::Stdout, b"boom\n"),
        (OutputStream::Stderr, b"keep this\n"),
    ];
    let test_cases = [
        ReplacementTestCase {
            description: "one chunk published changed, one filtered, one failed",
            output,
            discovery: Duration::ZERO,
            expected_transcript: b"KEEP THIS\n",
            expected_accounting: accounting.clone(),
        },
        ReplacementTestCase {
            description: "output written during a slow discovery waits for the replacement",
            output,
            discovery: Duration::from_millis(900),
            expected_transcript: b"KEEP THIS\n",
            expected_accounting: accounting,
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            replaced_session(test_case.output, test_case.discovery),
            (
                test_case.expected_transcript.to_vec(),
                test_case.expected_accounting
            ),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_one_table_of_a_batch_failing_when_written_then_only_its_records_count_as_failed() {
    let test_cases = [
        PartialInsertTestCase {
            description: "a refused output insert fails the output, not the events",
            failing: Table::Output,
            expected_counts: [(1, 1, 0), (1, 0, 1)],
        },
        PartialInsertTestCase {
            description: "a refused heartbeat insert fails no record",
            failing: Table::Heartbeats,
            expected_counts: [(1, 1, 0), (1, 1, 0)],
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            partially_inserted(test_case.failing),
            test_case.expected_counts,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_held_context_row_when_discovery_runs_then_the_first_heartbeat_names_the_command() {
    let test_cases = [DiscoveryTestCase {
        description: "a run killed during discovery is still listed with its command",
        wait: Duration::from_millis(600),
        expected_rows: vec![vec![
            RecallValue::Text("build".to_owned()),
            RecallValue::Text("running".to_owned()),
            RecallValue::Integer(1),
            RecallValue::Integer(0),
        ]],
    }];

    for test_case in test_cases {
        assert_eq!(
            invocation_during_discovery(test_case.wait),
            test_case.expected_rows,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_pending_events_when_replaced_on_the_delivery_thread_then_each_outcome_is_accounted() {
    let test_cases = [PendingEventTestCase {
        description: "one kept, one split in two, one filtered, one failed",
        copies: vec![Some(1), Some(2), Some(0), None],
        expected_counts: (5, 3, 1, 1),
    }];

    for test_case in test_cases {
        assert_eq!(
            pending_event_counts(&test_case.copies),
            test_case.expected_counts,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_store_from_the_first_release_when_written_then_rows_land_and_columns_are_added() {
    let test_cases = [
        CoreReleaseStoreTestCase {
            description: "a session adds the heartbeat columns and records the run",
            write: record_run,
            expected_rows: vec![vec![
                RecallValue::Integer(1),
                RecallValue::Text("build".to_owned()),
                RecallValue::Integer(2),
            ]],
        },
        CoreReleaseStoreTestCase {
            description: "a write before any upgrade stores only the columns the table has",
            write: insert_heartbeat,
            expected_rows: vec![vec![
                RecallValue::Integer(1),
                RecallValue::Null,
                RecallValue::Integer(0),
            ]],
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            written_to_core_release_store(test_case.write),
            test_case.expected_rows,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_events_of_mixed_priority_in_one_batch_when_replaced_then_the_sink_sees_publication_order()
{
    let test_cases = [ReplacementOrderTestCase {
        description: "a failure and a terminal event do not overtake earlier lifecycle events",
        priorities: vec![
            Priority::Lifecycle,
            Priority::FailedTerminal,
            Priority::Terminal,
            Priority::Lifecycle,
        ],
        expected_order: vec![0, 1, 2, 3],
    }];

    for test_case in test_cases {
        assert_eq!(
            replacement_order(&test_case.priorities),
            test_case.expected_order,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_byte_heavy_burst_when_the_window_is_long_then_it_is_sent_early_and_nothing_is_dropped() {
    let test_cases = [EarlyBatchTestCase {
        description: "12 MiB of output is sent long before a ten-minute window",
        written_bytes: 12 * 1024 * 1024,
        wait: Duration::from_secs(2),
        expected_minimum_sent_bytes: 8 * 1024 * 1024,
        expected_dropped: 0,
    }];

    for test_case in test_cases {
        let (sent, dropped) = sent_before_the_window(test_case.written_bytes, test_case.wait);
        assert_eq!(
            (sent >= test_case.expected_minimum_sent_bytes, dropped),
            (true, test_case.expected_dropped),
            "{}: {sent} bytes sent",
            test_case.description
        );
    }
}

#[test]
fn given_a_created_database_when_later_setup_fails_or_retries_then_its_notice_is_kept_once() {
    let test_cases = [
        CreationNoticeTestCase {
            description: "creation followed by refused DDL keeps the notice",
            failures: vec![Some(StoreErrorKind::Permission)],
            expected_information: vec!["created database `history`".to_owned()],
            expected_permission_denied: true,
        },
        CreationNoticeTestCase {
            description: "a busy retry after creation reports it once",
            failures: vec![Some(StoreErrorKind::Busy), None],
            expected_information: vec!["created database `history`".to_owned()],
            expected_permission_denied: false,
        },
        CreationNoticeTestCase {
            description: "a store opened at once reports the creation",
            failures: vec![None],
            expected_information: vec!["created database `history`".to_owned()],
            expected_permission_denied: false,
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            creation_notices(test_case.failures),
            (
                test_case.expected_information,
                test_case.expected_permission_denied
            ),
            "{}",
            test_case.description
        );
    }
}
