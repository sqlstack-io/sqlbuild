use crate::consumption::_helpers::rules::Shipped;
use crate::consumption::tests::helpers::found;
use crate::consumption::tests::test_types::{
    LatestRowTime, NoPlannerGuard, NoUpstreamRecursion, SimulationTestCase, TransitiveOnly, Writer,
};

#[test]
fn given_random_builds_crashes_and_appends_when_deciding_changed_then_only_broken_rules_miss() {
    let test_cases = [
        SimulationTestCase {
            description: "the shipped rules never skip a stale model or a pending replay",
            miss: found(&Shipped, Writer::Shipped),
            expected_miss: false,
        },
        SimulationTestCase {
            description: "without the started row a crash mid-write is skipped",
            miss: found(&Shipped, Writer::NoStartedRow),
            expected_miss: true,
        },
        SimulationTestCase {
            description: "comparing sources only misses a rebuilt upstream",
            miss: found(&TransitiveOnly, Writer::Shipped),
            expected_miss: true,
        },
        SimulationTestCase {
            description: "timing executions by their latest row lets a late duplicate hide a crash",
            miss: found(&LatestRowTime, Writer::Shipped),
            expected_miss: true,
        },
        SimulationTestCase {
            description: "without upstream recursion a stale unselected upstream is ignored",
            miss: found(&NoUpstreamRecursion, Writer::Shipped),
            expected_miss: true,
        },
        SimulationTestCase {
            description: "without the planner guard a pending replay is skipped",
            miss: found(&NoPlannerGuard, Writer::Shipped),
            expected_miss: true,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            test_case.miss.is_some(),
            test_case.expected_miss,
            "{}: {:?}",
            test_case.description,
            test_case.miss
        );
    }
}
