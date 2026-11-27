//! A large republish under request load keeps `/livez` answering, the server's memory within a bound of the snapshot's size, and no transaction idle.

use crate::api::tests::republish_load_helpers::measured_in_own_process;
use crate::api::tests::test_types::RepublishLoadTestCase;

#[cfg(target_os = "linux")]
#[test]
fn given_a_large_project_when_it_republishes_under_request_load_then_livez_answers_and_memory_and_transactions_stay_bounded()
 {
    let test_cases = [RepublishLoadTestCase {
        description: "2,300 models republished while people watch and read their node states",
        nodes: 2300,
        expected_growth_per_compressed_byte_at_most: 12,
        expected_slowest_livez_ms_at_most: 1000,
        expected_longest_idle_in_transaction_ms_at_most: 1000,
    }];
    for test_case in test_cases {
        let Some(observed) = measured_in_own_process(
            concat!(
                module_path!(),
                "::given_a_large_project_when_it_republishes_under_request_load_then_livez_answers_and_memory_and_transactions_stay_bounded"
            ),
            test_case.nodes,
        ) else {
            continue;
        };
        eprintln!("{}: {observed:?}", test_case.description);
        assert!(
            observed.caught_up && observed.livez_answers > 0,
            "{}: node states caught up {}, /livez answered {} times",
            test_case.description,
            observed.caught_up,
            observed.livez_answers
        );
        assert!(
            observed.slowest_livez_ms <= test_case.expected_slowest_livez_ms_at_most,
            "{}: /livez took {} ms",
            test_case.description,
            observed.slowest_livez_ms
        );
        assert!(
            observed.peak_growth_bytes
                <= observed.compressed_bytes
                    * test_case.expected_growth_per_compressed_byte_at_most,
            "{}: peak memory grew {} bytes for {} compressed bytes",
            test_case.description,
            observed.peak_growth_bytes,
            observed.compressed_bytes
        );
        assert!(
            observed.longest_idle_in_transaction_ms
                <= test_case.expected_longest_idle_in_transaction_ms_at_most,
            "{}: a session sat idle in a transaction for {} ms",
            test_case.description,
            observed.longest_idle_in_transaction_ms
        );
    }
}
