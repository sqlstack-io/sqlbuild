//! A failing sync waits at most a minute, never stops for an unreached warehouse, and resumes once the server answers.

use chrono::Utc;
use sqlbuild_orchestration::platform::main::work::connect_location_work::connect_location_work;
use sqlbuild_orchestration::warehouse_metadata::models::{MetadataBatch, WarehouseHistoryKind};
use tokio::time::Instant;

use crate::native_commands::_helpers::location::execution::metadata_sync;
use crate::native_commands::_helpers::location::sync_outcomes::deliver_metadata_page;
use crate::native_commands::models::LocationMetadataPage;
use crate::native_commands::tests::helpers::{answering_server, one_location, published_location};
use crate::native_commands::tests::test_types::{ServerRecoveryTestCase, UnreachedReadsTestCase};

#[test]
fn given_reads_that_never_reach_the_warehouse_when_retrying_then_they_wait_at_most_a_minute_and_never_stop()
 {
    let test_cases = [UnreachedReadsTestCase {
        description: "eight unreached reads, then five that reached the warehouse",
        unreached: 8,
        reached: 5,
        expected_waits: (
            vec![
                Some(30),
                Some(60),
                Some(60),
                Some(60),
                Some(60),
                Some(60),
                Some(60),
                Some(60),
            ],
            vec![Some(30), Some(60), Some(60), Some(60), None],
        ),
        expected_halted: true,
    }];
    for test_case in test_cases {
        let mut state = metadata_sync((one_location(), 1), (3600, 0));
        state.begin_sync(Utc::now());
        let unreached: Vec<Option<u64>> = (0..test_case.unreached)
            .map(|_| state.fail_read((false, true)).map(|wait| wait.as_secs()))
            .collect();
        let reached: Vec<Option<u64>> = (0..test_case.reached)
            .map(|_| state.fail_read((false, false)).map(|wait| wait.as_secs()))
            .collect();
        assert_eq!(
            ((unreached, reached), state.halted),
            (test_case.expected_waits, test_case.expected_halted),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_server_that_fails_then_answers_when_delivering_then_the_sync_resumes_by_itself() {
    let test_cases = [ServerRecoveryTestCase {
        description: "the page and its health report fail twice, then the server answers",
        statuses: &[503, 503, 503, 503],
        attempts: 3,
        expected_failing: vec![true, true, false],
        expected_line: "Run history sync recovered after 0 min; 1 page caught up.\n",
    }];
    for test_case in test_cases {
        let directory = tempfile::tempdir().expect("temporary directory");
        let ((options, _), (_, poll), mut state) = published_location(directory.path(), false);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let (url, _) = runtime.block_on(answering_server(test_case.statuses));
        let client =
            connect_location_work((&url, "token"), options.location_id).expect("work client");
        state.pending = Some(LocationMetadataPage {
            batch: MetadataBatch {
                kind: WarehouseHistoryKind::ALL[state.index],
                session_id: poll.session_id,
                generation: state.generation,
                records: vec![serde_json::json!({})],
                scan: None,
                last_page: false,
            },
            next_source_after: None,
        });
        let mut waits: Vec<u64> = Vec::new();
        let mut failing: Vec<bool> = Vec::new();
        for _ in 0..test_case.attempts {
            state = runtime
                .block_on(deliver_metadata_page(
                    state,
                    &options.target,
                    (&client, &poll),
                    (Instant::now(), std::future::pending()),
                ))
                .expect("no control error")
                .expect("not stopped");
            waits.push(
                state
                    .next_at
                    .saturating_duration_since(Instant::now())
                    .as_secs(),
            );
            failing.push(state.streak.failing_since.is_some());
            state.next_at = Instant::now();
        }
        assert_eq!(
            (
                waits.iter().all(|wait| *wait <= 60),
                failing,
                state.pending.is_none(),
                state.take_recovery_line(Utc::now()),
            ),
            (
                true,
                test_case.expected_failing,
                true,
                Some(test_case.expected_line.to_owned()),
            ),
            "{}",
            test_case.description
        );
    }
}
