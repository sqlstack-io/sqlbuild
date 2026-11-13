use crate::scheduler::main::_plan_ticks::plan_ticks;
use crate::scheduler::tests::helpers::{
    covered_seconds, request_seconds, skipped_seconds, state, time,
};
use crate::scheduler::tests::test_types::TickTestCase;

#[test]
fn given_durable_schedule_state_when_time_advances_then_decisions_preserve_due_ticks() {
    let test_cases = [
        TickTestCase {
            description: "catch-up requests only the latest missed time and records nothing earlier",
            due_seconds: vec![30, 10, 20, 10, 40],
            last_second: None,
            now_second: 30,
            active: false,
            running: false,
            expected_requests: vec![30],
            expected_covered: vec![],
            expected_skipped: vec![],
            expected_last: Some(30),
            expected_next: Some(40),
        },
        TickTestCase {
            description: "the latest missed time is covered by the already waiting run",
            due_seconds: vec![10, 20, 30, 40],
            last_second: Some(10),
            now_second: 30,
            active: true,
            running: false,
            expected_requests: vec![],
            expected_covered: vec![30],
            expected_skipped: vec![],
            expected_last: Some(30),
            expected_next: Some(40),
        },
        TickTestCase {
            description: "a backward clock jump cannot move the durable cursor backward",
            due_seconds: vec![10, 20, 30, 40],
            last_second: Some(30),
            now_second: 20,
            active: false,
            running: false,
            expected_requests: vec![],
            expected_covered: vec![],
            expected_skipped: vec![],
            expected_last: Some(30),
            expected_next: Some(40),
        },
        TickTestCase {
            description: "an on-time tick of a skip job yields to its running run",
            due_seconds: vec![10, 20, 30, 40],
            last_second: Some(20),
            now_second: 30,
            active: false,
            running: true,
            expected_requests: vec![],
            expected_covered: vec![],
            expected_skipped: vec![30],
            expected_last: Some(30),
            expected_next: Some(40),
        },
        TickTestCase {
            description: "a catch-up tick that collapsed missed times still queues while a skip job runs",
            due_seconds: vec![10, 20, 30, 40],
            last_second: Some(10),
            now_second: 30,
            active: false,
            running: true,
            expected_requests: vec![30],
            expected_covered: vec![],
            expected_skipped: vec![],
            expected_last: Some(30),
            expected_next: Some(40),
        },
        TickTestCase {
            description: "a waiting run covers the tick before a running run can skip it",
            due_seconds: vec![10, 20, 30, 40],
            last_second: Some(20),
            now_second: 30,
            active: true,
            running: true,
            expected_requests: vec![],
            expected_covered: vec![30],
            expected_skipped: vec![],
            expected_last: Some(30),
            expected_next: Some(40),
        },
    ];
    for test_case in test_cases {
        let (trigger, input) = state(&test_case);
        let plan =
            plan_ticks(&trigger, &input, time(test_case.now_second)).expect("plan valid schedule");
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
            skipped_seconds(&plan),
            test_case.expected_skipped,
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
        let retried =
            plan_ticks(&trigger, &input, time(test_case.now_second)).expect("retry schedule");
        assert_eq!(
            plan, retried,
            "a retry has exactly the same deterministic tick and run IDs"
        );
        let tick_ids: std::collections::BTreeSet<_> =
            plan.actions.iter().map(|action| action.tick_id).collect();
        assert_eq!(
            tick_ids.len(),
            test_case.expected_requests.len()
                + test_case.expected_covered.len()
                + test_case.expected_skipped.len()
        );
        let advanced = crate::scheduler::models::TickState {
            last_tick_for: plan.last_tick_for,
            ..input
        };
        let second = plan_ticks(&trigger, &advanced, time(test_case.now_second))
            .expect("plan from committed cursor");
        assert!(second.actions.iter().all(|next| {
            plan.actions
                .iter()
                .all(|before| before.tick_id != next.tick_id)
        }));
    }
}
