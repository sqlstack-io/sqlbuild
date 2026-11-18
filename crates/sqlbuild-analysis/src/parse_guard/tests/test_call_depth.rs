use crate::parse_guard::constants::MAX_FUNCTION_CALL_DEPTH;
use crate::parse_guard::tests::helpers::{nested_calls, on_default_stack, parses};
use crate::parse_guard::tests::test_types::CallDepthTestCase;

#[test]
fn given_nested_calls_when_parsing_on_a_default_stack_then_each_parser_applies_the_shared_limit() {
    let test_cases = [
        CallDepthTestCase {
            description: "one past the libraries' default of 64",
            depth: 65,
            expected_error: None,
        },
        CallDepthTestCase {
            description: "a hundred deep",
            depth: 100,
            expected_error: None,
        },
        CallDepthTestCase {
            description: "exactly the limit",
            depth: MAX_FUNCTION_CALL_DEPTH,
            expected_error: None,
        },
        CallDepthTestCase {
            description: "one past the limit names it",
            depth: MAX_FUNCTION_CALL_DEPTH + 1,
            expected_error: Some(format!(
                "E_GUARD_FUNCTION_NESTING_DEPTH_EXCEEDED: value {} exceeds configured limit \
                 {MAX_FUNCTION_CALL_DEPTH}",
                MAX_FUNCTION_CALL_DEPTH + 1
            )),
        },
    ];

    for test_case in test_cases {
        let sql: String = nested_calls(test_case.depth);
        let expected_length: usize = test_case.expected_error.as_ref().map_or(0, String::len);

        for (parser, outcome) in on_default_stack(|| parses(&sql)) {
            let guard_error: Option<String> = outcome.err().map(|error| {
                error
                    .get(error.find("E_GUARD").unwrap_or(0)..)
                    .unwrap_or_default()
                    .chars()
                    .take(expected_length)
                    .collect()
            });

            assert_eq!(
                guard_error, test_case.expected_error,
                "{}: {parser}",
                test_case.description
            );
        }
    }
}
