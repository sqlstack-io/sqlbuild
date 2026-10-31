use std::collections::BTreeSet;
use std::num::NonZeroUsize;
use std::time::{Duration, Instant};

use crate::declarations::models::{Schedule, TriggerDefinition, TriggerStatus};
use crate::scheduler::_helpers::expansion::expand_schedule;
use crate::scheduler::constants::MAX_TICKS_PER_BATCH;
use crate::scheduler::main::_plan_ticks::plan_ticks;
use crate::scheduler::models::{CatchUp, Overlap, TickDecision, TickState};
use crate::scheduler::tests::helpers::time;
use crate::scheduler::tests::test_types::BacklogTestCase;
use uuid::Uuid;

#[test]
fn given_a_backlog_larger_than_two_batches_when_drained_then_latest_is_current_and_every_tick_is_retained()
 {
    let test_cases = [
        BacklogTestCase {
            description: "latest records covered prefixes without requesting an obsolete run",
            catch_up: CatchUp::Latest,
            expected_latest: 1_200_180,
            expected_count: 20_003,
        },
        BacklogTestCase {
            description: "all drains every requested tick in bounded batches",
            catch_up: CatchUp::All {
                max_runs: NonZeroUsize::new(MAX_TICKS_PER_BATCH).expect("nonzero bound"),
            },
            expected_latest: 1_200_180,
            expected_count: 20_003,
        },
    ];
    for test_case in test_cases {
        let started = Instant::now();
        let trigger = TriggerDefinition {
            description: "orders backlog".into(),
            job_key: "orders".into(),
            starts_at: Some(time(60)),
            default_status: TriggerStatus::Running,
            catch_up: test_case.catch_up,
            schedule: Schedule::Cron {
                expression: "* * * * *".into(),
                timezone: "UTC".into(),
            },
        };
        let mut cursor = None;
        let mut recorded: BTreeSet<i64> = BTreeSet::new();
        let mut requested: BTreeSet<i64> = BTreeSet::new();
        for _ in 0..4 {
            let (due_times, continuation) = expand_schedule(
                &trigger,
                cursor.unwrap_or(time(0)),
                time(test_case.expected_latest),
            )
            .expect("bounded expansion");
            let plan = plan_ticks(
                &TickState {
                    trigger_id: Uuid::from_u128(19),
                    enabled: true,
                    last_tick_for: cursor,
                    due_times,
                    continuation,
                    catch_up: test_case.catch_up,
                    overlap: Overlap::Queue,
                    waiting_run: None,
                },
                time(test_case.expected_latest),
            )
            .expect("bounded plan");
            for action in plan.actions {
                recorded.insert(action.scheduled_for.timestamp());
                let TickDecision::Requested { .. } = action.decision else {
                    continue;
                };
                requested.insert(action.scheduled_for.timestamp());
            }
            cursor = plan.last_tick_for;
        }
        assert_eq!(
            recorded,
            (1..=test_case.expected_count)
                .map(|minute| minute as i64 * 60)
                .collect(),
            "{}",
            test_case.description
        );
        assert_eq!(
            requested.last().copied(),
            Some(test_case.expected_latest),
            "{}",
            test_case.description
        );
        let expected_requests =
            [test_case.expected_count, 1][usize::from(test_case.catch_up == CatchUp::Latest)];
        assert_eq!(
            requested.len(),
            expected_requests,
            "{}",
            test_case.description
        );
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "bounded cron expansion exceeded five seconds: {}",
            test_case.description
        );
    }
}
