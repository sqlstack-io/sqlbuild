use crate::parse_guard::constants::MAX_FUNCTION_CALL_DEPTH;
use crate::sql_macros::tests::helpers::{expanded_for, nested_replace_macro};
use crate::sql_macros::tests::test_types::NestedMacroTestCase;

#[test]
fn given_a_deeply_nested_macro_body_when_expanding_for_snowflake_then_the_shared_limit_applies() {
    let test_cases = [
        NestedMacroTestCase {
            description: "a hundred nested calls expand",
            depth: 100,
            expected: Ok(100),
        },
        NestedMacroTestCase {
            description: "one past the limit is P015 naming the limit",
            depth: MAX_FUNCTION_CALL_DEPTH + 1,
            expected: Err(format!(
                "body of macro cleaned_name() is not a valid expression macro: \
                 E_GUARD_FUNCTION_NESTING_DEPTH_EXCEEDED: value {} exceeds configured limit \
                 {MAX_FUNCTION_CALL_DEPTH}",
                MAX_FUNCTION_CALL_DEPTH + 1
            )),
        },
    ];

    for test_case in test_cases {
        let source: String = nested_replace_macro(test_case.depth);

        let outcome = expanded_for(
            "snowflake",
            &[("cleaned_name", &source)],
            "SELECT @sql.cleaned_name(race_name) AS cleaned FROM races",
        )
        .map(|sql| sql.matches("REPLACE(").count())
        .map_err(|error| format!("{}: {}", error.code, error.message));

        assert_eq!(
            outcome,
            test_case
                .expected
                .map_err(|message| format!("P015: {message}")),
            "{}",
            test_case.description
        );
    }
}
