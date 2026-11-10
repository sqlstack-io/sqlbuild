use std::collections::HashMap;

use crate::compile_timings::main::compile_timing_milliseconds::compile_timing_milliseconds;
use crate::compile_timings::models::CompileTimings;
use crate::compile_timings::tests::test_types::MillisecondsTestCase;

#[test]
fn given_recorded_timings_when_reported_then_lists_every_phase_and_metric_in_order() {
    let test_cases = [
        MillisecondsTestCase {
            description: "nothing recorded reports zeros",
            timings: CompileTimings::default(),
            expected_entries: vec![("attachment_ms", 0), ("sql_test_scan_cache_misses", 0)],
        },
        MillisecondsTestCase {
            description: "nanoseconds floor to milliseconds and unknown phases are dropped",
            timings: CompileTimings {
                elapsed_ns: HashMap::from([
                    ("attachment_ms".to_owned(), 2_999_999),
                    ("unknown_ms".to_owned(), 5_000_000),
                ]),
                metrics: HashMap::from([("sql_test_scan_cache_misses".to_owned(), 3)]),
            },
            expected_entries: vec![("attachment_ms", 2), ("sql_test_scan_cache_misses", 3)],
        },
    ];
    for test_case in test_cases {
        let entries = compile_timing_milliseconds(&test_case.timings);
        assert_eq!(entries.len(), 25, "{}", test_case.description);
        assert_eq!(
            vec![entries[0], entries[20]],
            test_case.expected_entries,
            "{}",
            test_case.description
        );
    }
}
