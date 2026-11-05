use crate::consumption::tests::helpers::found;
use crate::consumption::tests::test_types::{SimulationTestCase, Writer};

#[test]
fn given_random_builds_crashes_and_appends_when_deciding_changed_then_only_a_broken_writer_misses()
{
    let test_cases = [
        SimulationTestCase {
            description: "the shipped decision never skips a stale model or a pending replay",
            miss: found(Writer::Shipped),
            expected_miss: false,
        },
        SimulationTestCase {
            description: "without the started row a crash mid-write is skipped",
            miss: found(Writer::NoStartedRow),
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
