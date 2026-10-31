use std::num::NonZeroUsize;

use crate::scheduler::main::_plan_ticks::plan_ticks;
use crate::scheduler::models::{CatchUp, Overlap};
use crate::scheduler::tests::helpers::{covered_seconds, request_seconds, state, time};
use crate::scheduler::tests::test_types::TickTestCase;

#[test]
fn given_durable_schedule_state_when_time_advances_then_decisions_preserve_due_ticks() {
    let test_cases = [
        TickTestCase {
            description: "latest catch-up covers earlier ticks and deduplicates calendar entries",
            enabled: true,
            due_seconds: vec![30, 10, 20, 10, 40],
            last_second: None,
            now_second: 30,
            catch_up: CatchUp::Latest,
            overlap: Overlap::Coalesce,
            active: false,
            expected_requests: vec![30],
            expected_covered: vec![10, 20],
            expected_last: Some(30),
            expected_next: Some(40),
        },
        TickTestCase {
            description: "all catch-up leaves the unprocessed backlog due instead of skipping it",
            enabled: true,
            due_seconds: vec![10, 20, 30, 40],
            last_second: None,
            now_second: 40,
            catch_up: CatchUp::All {
                max_runs: NonZeroUsize::new(2).expect("nonzero test batch"),
            },
            overlap: Overlap::Queue,
            active: false,
            expected_requests: vec![10, 20],
            expected_covered: vec![],
            expected_last: Some(20),
            expected_next: Some(30),
        },
        TickTestCase {
            description: "coalesced ticks are covered by the already waiting run",
            enabled: true,
            due_seconds: vec![10, 20, 30, 40],
            last_second: Some(10),
            now_second: 30,
            catch_up: CatchUp::Latest,
            overlap: Overlap::Coalesce,
            active: true,
            expected_requests: vec![],
            expected_covered: vec![20, 30],
            expected_last: Some(30),
            expected_next: Some(40),
        },
        TickTestCase {
            description: "allow does not coalesce even when a prior run is active",
            enabled: true,
            due_seconds: vec![10, 20, 30, 40],
            last_second: Some(10),
            now_second: 30,
            catch_up: CatchUp::All {
                max_runs: NonZeroUsize::new(5).expect("nonzero test batch"),
            },
            overlap: Overlap::Allow,
            active: true,
            expected_requests: vec![20, 30],
            expected_covered: vec![],
            expected_last: Some(30),
            expected_next: Some(40),
        },
        TickTestCase {
            description: "a backward clock jump cannot move the durable cursor backward",
            enabled: true,
            due_seconds: vec![10, 20, 30, 40],
            last_second: Some(30),
            now_second: 20,
            catch_up: CatchUp::Latest,
            overlap: Overlap::Coalesce,
            active: false,
            expected_requests: vec![],
            expected_covered: vec![],
            expected_last: Some(30),
            expected_next: Some(40),
        },
        TickTestCase {
            description: "pause retains the cursor and does not advertise an overdue next tick",
            enabled: false,
            due_seconds: vec![10, 20, 30],
            last_second: Some(10),
            now_second: 30,
            catch_up: CatchUp::Latest,
            overlap: Overlap::Coalesce,
            active: false,
            expected_requests: vec![],
            expected_covered: vec![],
            expected_last: Some(10),
            expected_next: None,
        },
    ];
    for test_case in test_cases {
        let input = state(&test_case);
        let plan = plan_ticks(&input, time(test_case.now_second)).expect("plan valid schedule");
        assert_eq!(
            request_seconds(&plan),
            test_case.expected_requests,
            "{}",
            test_case.description
        );
        assert_eq!(
            covered_seconds(&plan),
            test_case.expected_covered,
            "{}",
            test_case.description
        );
        assert_eq!(
            plan.last_tick_for.map(|time| time.timestamp()),
            test_case.expected_last
        );
        assert_eq!(
            plan.next_due_at.map(|time| time.timestamp()),
            test_case.expected_next
        );
        let retried = plan_ticks(&input, time(test_case.now_second)).expect("retry schedule");
        assert_eq!(
            plan, retried,
            "a retry has exactly the same deterministic tick and run IDs"
        );
        let tick_ids: std::collections::BTreeSet<_> =
            plan.actions.iter().map(|action| action.tick_id).collect();
        assert_eq!(
            tick_ids.len(),
            test_case.expected_requests.len() + test_case.expected_covered.len()
        );
        let advanced = crate::scheduler::models::TickState {
            last_tick_for: plan.last_tick_for,
            ..input
        };
        let second =
            plan_ticks(&advanced, time(test_case.now_second)).expect("plan from committed cursor");
        assert!(second.actions.iter().all(|next| {
            plan.actions
                .iter()
                .all(|before| before.tick_id != next.tick_id)
        }));
    }
}
