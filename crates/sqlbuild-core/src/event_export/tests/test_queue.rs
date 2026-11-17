use crate::event_export::main::queue_lifecycle_event::queue_lifecycle_event;
use crate::event_export::main::retain_outcome_events::retain_outcome_events;
use crate::event_export::main::take_lifecycle_event::take_lifecycle_event;
use crate::event_export::models::EventQueue;
use crate::event_export::tests::helpers::{event, invocation_event, queued, sequences};
use crate::event_export::tests::test_types::{
    DeliveryOrderTestCase, DisplacementTestCase, InvocationStartOrderTestCase,
    OutcomeAdmissionTestCase, OutcomeReserveTestCase,
};

#[test]
fn given_full_queue_when_outcome_or_other_event_arrives_then_outcomes_are_admitted_and_kept() {
    let test_cases = [
        OutcomeAdmissionTestCase {
            description: "outcome displaces an equal-priority event",
            queued: &[(2, false), (2, false)],
            incoming: (2, true),
            expected_inserted: true,
            expected_displaced: Some(0),
            expected_sequences: &[1, 2],
        },
        OutcomeAdmissionTestCase {
            description: "outcome displaces a higher-priority event when nothing lower is queued",
            queued: &[(3, false), (2, true)],
            incoming: (0, true),
            expected_inserted: true,
            expected_displaced: Some(0),
            expected_sequences: &[1, 2],
        },
        OutcomeAdmissionTestCase {
            description: "outcome exceeds capacity when only outcomes are queued",
            queued: &[(2, true), (0, true)],
            incoming: (2, true),
            expected_inserted: true,
            expected_displaced: None,
            expected_sequences: &[0, 1, 2],
        },
        OutcomeAdmissionTestCase {
            description: "failure never displaces a queued outcome",
            queued: &[(2, true), (0, true)],
            incoming: (3, false),
            expected_inserted: false,
            expected_displaced: None,
            expected_sequences: &[0, 1],
        },
    ];

    for test_case in test_cases {
        let mut queue: EventQueue<()> = EventQueue::new(test_case.queued.len());
        for (sequence, dimensions) in (0_u64..).zip(test_case.queued) {
            queue = queue_lifecycle_event(queue, event(*dimensions, sequence)).queue;
        }
        let incoming_sequence: u64 = u64::try_from(test_case.queued.len()).unwrap_or(u64::MAX);
        let outcome = queue_lifecycle_event(queue, event(test_case.incoming, incoming_sequence));
        assert_eq!(
            (
                outcome.inserted,
                outcome.displaced.as_ref().map(|event| event.sequence),
                sequences(&outcome.queue),
            ),
            (
                test_case.expected_inserted,
                test_case.expected_displaced,
                test_case.expected_sequences.to_vec(),
            ),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_mixed_backlog_when_reserve_begins_then_only_outcome_events_remain() {
    let test_cases = [OutcomeReserveTestCase {
        description: "other events are handed back for accounting",
        queued: &[(2, false), (2, true), (0, false), (2, true)],
        expected_kept: &[1, 3],
        expected_dropped: &[0, 2],
    }];

    for test_case in test_cases {
        let mut queue: EventQueue<()> = EventQueue::new(8);
        for (sequence, dimensions) in (0_u64..).zip(test_case.queued) {
            queue = queue_lifecycle_event(queue, event(*dimensions, sequence)).queue;
        }

        let (kept, dropped) = retain_outcome_events(queue);

        assert_eq!(
            (
                sequences(&kept),
                dropped
                    .iter()
                    .map(|item| item.sequence)
                    .collect::<Vec<u64>>()
            ),
            (
                test_case.expected_kept.to_vec(),
                test_case.expected_dropped.to_vec()
            ),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_full_queue_when_priority_pair_arrives_then_displacement_policy_is_exact() {
    let test_cases = [
        DisplacementTestCase {
            description: "equal priority is rejected",
            priorities: (2, 2),
            expected_inserted: false,
            expected_displaced: None,
            expected_retained: 1,
        },
        DisplacementTestCase {
            description: "lower priority is rejected",
            priorities: (3, 0),
            expected_inserted: false,
            expected_displaced: None,
            expected_retained: 1,
        },
        DisplacementTestCase {
            description: "higher priority displaces the queued event",
            priorities: (0, 1),
            expected_inserted: true,
            expected_displaced: Some(1),
            expected_retained: 2,
        },
    ];

    for test_case in test_cases {
        let (queued_priority, incoming_priority) = test_case.priorities;
        let first = queue_lifecycle_event(EventQueue::new(1), queued(1, (queued_priority, false)));
        let outcome = queue_lifecycle_event(first.queue, queued(2, (incoming_priority, false)));
        let (_, retained) = take_lifecycle_event(outcome.queue);
        let retained = retained.map(|event| event.sequence);
        assert!(first.inserted, "{}", test_case.description);
        assert_eq!(
            (
                outcome.inserted,
                outcome.displaced.map(|event| event.sequence),
                retained
            ),
            (
                test_case.expected_inserted,
                test_case.expected_displaced,
                Some(test_case.expected_retained)
            ),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_queued_events_when_taking_then_priority_then_fifo_with_invocation_terminals_last() {
    let test_cases = [
        DeliveryOrderTestCase {
            description: "highest priority first and FIFO within a priority",
            events: &[(0, false), (3, false), (2, false), (3, false), (0, false)],
            expected_sequences: &[1, 3, 2, 0, 4],
        },
        DeliveryOrderTestCase {
            description: "invocation terminals wait behind lower-priority events",
            events: &[(3, true), (0, false), (1, false)],
            expected_sequences: &[2, 1, 0],
        },
    ];

    for test_case in test_cases {
        let mut queue: EventQueue<()> = EventQueue::new(16);
        for (sequence, event) in (0_u64..).zip(test_case.events) {
            queue = queue_lifecycle_event(queue, queued(sequence, *event)).queue;
        }
        let mut order: Vec<u64> = Vec::new();
        for _ in test_case.events {
            let (rest, next) = take_lifecycle_event(queue);
            queue = rest;
            order.extend(next.map(|event| event.sequence));
        }
        assert_eq!(
            order, test_case.expected_sequences,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_queued_invocation_start_when_taking_then_it_precedes_only_its_own_invocation() {
    let test_cases = [
        InvocationStartOrderTestCase {
            description: "a start goes before its invocation's higher-priority events",
            events: &[(0, true, "a"), (2, false, "a"), (3, false, "a")],
            expected_sequences: &[0, 2, 1],
        },
        InvocationStartOrderTestCase {
            description: "a start that is not queued holds nothing back",
            events: &[(2, false, "a"), (0, false, "a"), (3, false, "a")],
            expected_sequences: &[2, 0, 1],
        },
        InvocationStartOrderTestCase {
            description: "one invocation's start does not jump another's events",
            events: &[
                (2, false, "b"),
                (0, true, "a"),
                (2, false, "a"),
                (0, false, "b"),
            ],
            expected_sequences: &[0, 1, 2, 3],
        },
    ];

    for test_case in test_cases {
        let mut queue: EventQueue<()> = EventQueue::new(16);
        for (sequence, event) in (0_u64..).zip(test_case.events) {
            queue = queue_lifecycle_event(queue, invocation_event(sequence, *event)).queue;
        }
        let mut order: Vec<u64> = Vec::new();
        for _ in test_case.events {
            let (rest, next) = take_lifecycle_event(queue);
            queue = rest;
            order.extend(next.map(|event| event.sequence));
        }
        assert_eq!(
            order, test_case.expected_sequences,
            "{}",
            test_case.description
        );
    }
}
