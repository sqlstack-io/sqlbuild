use crate::engine::tests::{helpers, test_types};
use serde_json::json;
use tempfile::TempDir;

const EMPTY_ORDERS: (&str, &str) = (
    "__source__raw_orders",
    "SELECT NULL AS order_id, NULL AS amount WHERE FALSE",
);
const ORDER_ROWS: (&str, &str) = (
    "__source__raw_orders",
    "SELECT 1 AS order_id, 5 AS amount UNION ALL SELECT 2 AS order_id, 7 AS amount",
);
const NO_ROWS_ASSERTION: (&str, &str) = (
    "__assert__empty_inputs_produce_no_rows",
    "SELECT 1 AS unexpected_row FROM __ref(\"orders\")",
);
const UNAVAILABLE_MESSAGE: &str = "L007 tests/unit/test_orders__empty_inputs_produce_no_rows.sql: SQLBuild cannot parse mssql SQL for built-in rules yet, so these rules were not evaluated: SQBRTEST203";
const FLAGGED_MESSAGE: &str = "SQBRTEST203 tests/unit/test_orders__empty_inputs_produce_no_rows.sql: unit test block 1 (\"orders__empty_inputs_produce_no_rows\") mocks only empty inputs and asserts only that no rows are produced";

#[test]
fn given_sql_test_shapes_when_evaluating_empty_input_rule_then_flags_only_filler_tests()
-> Result<(), String> {
    let project_dir = TempDir::new().map_err(|error| error.to_string())?;
    let test_cases = [
        test_types::EmptyInputTestRuleTestCase {
            description: "all-empty mocks with a no-rows assertion are flagged",
            test: helpers::filler_test(),
            allowed_tests: json!([]),
            expected_messages: &[FLAGGED_MESSAGE],
        },
        test_types::EmptyInputTestRuleTestCase {
            description: "all-empty mocks with an empty expected output are flagged",
            test: helpers::empty_input_test_fact(
                "orders__empty_inputs_produce_no_rows",
                &[
                    (
                        "__source__raw_orders",
                        "SELECT NULL AS order_id, NULL AS amount LIMIT 0",
                    ),
                    ("__ref__customers", "SELECT * FROM __EMPTY_FIXTURE()"),
                ],
                &[("__expected__orders", "SELECT * FROM __empty_fixture()")],
                &[],
            ),
            allowed_tests: json!([]),
            expected_messages: &[FLAGGED_MESSAGE],
        },
        test_types::EmptyInputTestRuleTestCase {
            description: "a cast to a parameterized type under a false predicate is an empty mock",
            test: helpers::empty_input_test_fact(
                "orders__empty_inputs_produce_no_rows",
                &[(
                    "__source__raw_orders",
                    "SELECT CAST(NULL AS DECIMAL(18, 2)) AS amount WHERE 1 = 0",
                )],
                &[],
                &[NO_ROWS_ASSERTION],
            ),
            allowed_tests: json!([]),
            expected_messages: &[FLAGGED_MESSAGE],
        },
        test_types::EmptyInputTestRuleTestCase {
            description: "a cast to a parameterized type keeps a bare existence check",
            test: helpers::empty_input_test_fact(
                "orders__empty_inputs_produce_no_rows",
                &[EMPTY_ORDERS],
                &[],
                &[(
                    "__assert__empty_inputs_produce_no_rows",
                    "SELECT CAST(order_id AS VARCHAR(10)) AS unexpected_row FROM __ref(\"orders\")",
                )],
            ),
            allowed_tests: json!([]),
            expected_messages: &[FLAGGED_MESSAGE],
        },
        test_types::EmptyInputTestRuleTestCase {
            description: "input rows with expected output pass",
            test: helpers::empty_input_test_fact(
                "orders__doubles_amounts",
                &[ORDER_ROWS],
                &[(
                    "__expected__orders",
                    "SELECT 1 AS order_id, 10 AS doubled_amount",
                )],
                &[],
            ),
            allowed_tests: json!([]),
            expected_messages: &[],
        },
        test_types::EmptyInputTestRuleTestCase {
            description: "input rows with an existence assertion pass",
            test: helpers::empty_input_test_fact(
                "orders__keeps_every_order",
                &[ORDER_ROWS],
                &[],
                &[NO_ROWS_ASSERTION],
            ),
            allowed_tests: json!([]),
            expected_messages: &[],
        },
        test_types::EmptyInputTestRuleTestCase {
            description: "one empty mock alongside a mock with rows passes",
            test: helpers::empty_input_test_fact(
                "orders__ignores_missing_customers",
                &[
                    ORDER_ROWS,
                    ("__ref__customers", "SELECT NULL AS customer_id WHERE 1 = 0"),
                ],
                &[],
                &[NO_ROWS_ASSERTION],
            ),
            allowed_tests: json!([]),
            expected_messages: &[],
        },
        test_types::EmptyInputTestRuleTestCase {
            description: "a union mock whose last branch is filtered is not empty",
            test: helpers::empty_input_test_fact(
                "orders__keeps_first_branch",
                &[(
                    "__source__raw_orders",
                    "SELECT 1 AS order_id, 5 AS amount UNION ALL SELECT 2 AS order_id, 7 AS amount WHERE FALSE",
                )],
                &[],
                &[NO_ROWS_ASSERTION],
            ),
            allowed_tests: json!([]),
            expected_messages: &[],
        },
        test_types::EmptyInputTestRuleTestCase {
            description: "all-empty mocks with a summary-row EXCEPT assertion pass",
            test: helpers::empty_input_test_fact(
                "orders__reports_zero_summary",
                &[EMPTY_ORDERS],
                &[],
                &[(
                    "__assert__zero_summary_row",
                    "SELECT order_count, total_amount FROM __ref(\"orders\") EXCEPT SELECT 0 AS order_count, 0 AS total_amount",
                )],
            ),
            allowed_tests: json!([]),
            expected_messages: &[],
        },
        test_types::EmptyInputTestRuleTestCase {
            description: "all-empty mocks with a filtered assertion pass",
            test: helpers::empty_input_test_fact(
                "orders__never_negative",
                &[EMPTY_ORDERS],
                &[],
                &[(
                    "__assert__no_negative_amounts",
                    "SELECT 1 FROM __ref(\"orders\") WHERE doubled_amount < 0",
                )],
            ),
            allowed_tests: json!([]),
            expected_messages: &[],
        },
        test_types::EmptyInputTestRuleTestCase {
            description: "all-empty mocks with an aggregate-projection assertion pass",
            test: helpers::empty_input_test_fact(
                "orders__reports_one_zero_summary",
                &[EMPTY_ORDERS],
                &[],
                &[(
                    "__assert__one_zero_summary_row",
                    "SELECT UNNEST(CASE WHEN COUNT(*) = 1 AND MIN(order_count) = 0 THEN [] ELSE [1] END) AS failure FROM __ref(\"orders\")",
                )],
            ),
            allowed_tests: json!([]),
            expected_messages: &[],
        },
        test_types::EmptyInputTestRuleTestCase {
            description: "all-empty mocks with an offset assertion pass",
            test: helpers::empty_input_test_fact(
                "orders__reports_at_most_one_summary",
                &[EMPTY_ORDERS],
                &[],
                &[(
                    "__assert__at_most_one_summary_row",
                    "SELECT 1 AS unexpected_extra_summary FROM __ref(\"orders\") OFFSET 1",
                )],
            ),
            allowed_tests: json!([]),
            expected_messages: &[],
        },
        test_types::EmptyInputTestRuleTestCase {
            description: "a filtered mock ordered by an aggregate is not empty",
            test: helpers::empty_input_test_fact(
                "orders__drops_first_order",
                &[(
                    "__source__raw_orders",
                    "SELECT 1 AS order_id, 5 AS amount WHERE FALSE ORDER BY COUNT(*)",
                )],
                &[],
                &[NO_ROWS_ASSERTION],
            ),
            allowed_tests: json!([]),
            expected_messages: &[],
        },
        test_types::EmptyInputTestRuleTestCase {
            description: "an allowlisted filler test passes",
            test: helpers::filler_test(),
            allowed_tests: json!(["orders__empty_inputs_produce_no_rows"]),
            expected_messages: &[],
        },
        test_types::EmptyInputTestRuleTestCase {
            description: "a stale allowlist entry is reported",
            test: helpers::empty_input_test_fact(
                "orders__doubles_amounts",
                &[ORDER_ROWS],
                &[(
                    "__expected__orders",
                    "SELECT 1 AS order_id, 10 AS doubled_amount",
                )],
                &[],
            ),
            allowed_tests: json!(["orders__retired_filler"]),
            expected_messages: &[
                "SQBRTEST203 sqlbuild_project.toml: stale allowed_tests entry \"orders__retired_filler\" names no existing SQL test",
            ],
        },
        test_types::EmptyInputTestRuleTestCase {
            description: "an unparseable mock reports missing evaluation evidence",
            test: helpers::empty_input_test_fact(
                "orders__empty_inputs_produce_no_rows",
                &[(
                    "__source__raw_orders",
                    "SELECT NULL AS order_id WHERE FALSE LIMIT",
                )],
                &[],
                &[NO_ROWS_ASSERTION],
            ),
            allowed_tests: json!([]),
            expected_messages: &[
                "SQBRTEST203 tests/unit/test_orders__empty_inputs_produce_no_rows.sql: Rule SQBRTEST203 could not be evaluated for test block 1: could not parse SQL-test fixture: Parse error at line 1, column 42: Expected an expression after LIMIT",
            ],
        },
        test_types::EmptyInputTestRuleTestCase {
            description: "a macro-mode test is not flagged",
            test: helpers::with_field(helpers::filler_test(), "mode", json!("macro")),
            allowed_tests: json!([]),
            expected_messages: &[],
        },
        test_types::EmptyInputTestRuleTestCase {
            description: "a test with macro mocks is not flagged",
            test: helpers::with_field(helpers::filler_test(), "has_macro_mocks", json!(true)),
            allowed_tests: json!([]),
            expected_messages: &[],
        },
        test_types::EmptyInputTestRuleTestCase {
            description: "a test with model query overrides is not flagged",
            test: helpers::with_field(
                helpers::filler_test(),
                "has_model_query_overrides",
                json!(true),
            ),
            allowed_tests: json!([]),
            expected_messages: &[],
        },
        test_types::EmptyInputTestRuleTestCase {
            description: "a global aggregate under a false predicate is not an empty mock",
            test: helpers::empty_input_test_fact(
                "orders__empty_inputs_produce_no_rows",
                &[(
                    "__source__raw_orders",
                    "SELECT COUNT(*) AS order_id, 0 AS amount WHERE FALSE",
                )],
                &[],
                &[NO_ROWS_ASSERTION],
            ),
            allowed_tests: json!([]),
            expected_messages: &[],
        },
        test_types::EmptyInputTestRuleTestCase {
            description: "an assertion on an untested model is not a bare existence check",
            test: helpers::empty_input_test_fact(
                "orders__empty_inputs_produce_no_rows",
                &[EMPTY_ORDERS],
                &[],
                &[(
                    "__assert__no_customers",
                    "SELECT 1 FROM __ref(\"customers\")",
                )],
            ),
            allowed_tests: json!([]),
            expected_messages: &[],
        },
        test_types::EmptyInputTestRuleTestCase {
            description: "an assertion with a subquery is not a bare existence check",
            test: helpers::empty_input_test_fact(
                "orders__empty_inputs_produce_no_rows",
                &[EMPTY_ORDERS],
                &[],
                &[(
                    "__assert__no_rows",
                    "SELECT (SELECT 1) AS unexpected_row FROM __ref(\"orders\")",
                )],
            ),
            allowed_tests: json!([]),
            expected_messages: &[],
        },
    ];

    for test_case in test_cases {
        let messages = helpers::empty_input_rule_evaluation(
            &project_dir,
            json!(["SQBRTEST203"]),
            json!([test_case.test]),
            test_case.allowed_tests,
            false,
        )?;
        assert_eq!(
            messages, test_case.expected_messages,
            "{}",
            test_case.description
        );
    }
    Ok(())
}

#[test]
fn given_empty_input_only_tests_when_evaluating_minimum_tests_then_they_do_not_count()
-> Result<(), String> {
    let project_dir = TempDir::new().map_err(|error| error.to_string())?;
    let real_test = helpers::empty_input_test_fact(
        "orders__doubles_amounts",
        &[ORDER_ROWS],
        &[(
            "__expected__orders",
            "SELECT 1 AS order_id, 10 AS doubled_amount",
        )],
        &[],
    );
    let test_cases = [
        test_types::EmptyInputMinimumTestsTestCase {
            description: "filler is not counted when only SQBRTEST202 is selected",
            select: json!(["SQBRTEST202"]),
            tests: json!([helpers::filler_test()]),
            allowed_tests: json!([]),
            expected_messages: &[
                "SQBRTEST202 models/orders.sql: model \"orders\" has 0 tests; 1 required (1 empty-input-only test not counted; see SQBRTEST203)",
            ],
        },
        test_types::EmptyInputMinimumTestsTestCase {
            description: "a real test satisfies the minimum alongside filler",
            select: json!(["SQBRTEST202"]),
            tests: json!([helpers::filler_test(), real_test]),
            allowed_tests: json!([]),
            expected_messages: &[],
        },
        test_types::EmptyInputMinimumTestsTestCase {
            description: "an allowlisted filler test counts toward the minimum",
            select: json!(["SQBRTEST202", "SQBRTEST203"]),
            tests: json!([helpers::filler_test()]),
            allowed_tests: json!(["orders__empty_inputs_produce_no_rows"]),
            expected_messages: &[],
        },
    ];

    for test_case in test_cases {
        let messages = helpers::empty_input_rule_evaluation(
            &project_dir,
            test_case.select,
            test_case.tests,
            test_case.allowed_tests,
            false,
        )?;
        assert_eq!(
            messages, test_case.expected_messages,
            "{}",
            test_case.description
        );
    }
    Ok(())
}

#[test]
fn given_cached_minimum_tests_when_filler_becomes_real_test_then_model_cache_is_invalidated()
-> Result<(), String> {
    let project_dir = TempDir::new().map_err(|error| error.to_string())?;
    let real_test = helpers::empty_input_test_fact(
        "orders__empty_inputs_produce_no_rows",
        &[ORDER_ROWS],
        &[],
        &[NO_ROWS_ASSERTION],
    );
    let test_cases = [
        test_types::EmptyInputMinimumTestsTestCase {
            description: "filler fails the cached minimum",
            select: json!(["SQBRTEST202"]),
            tests: json!([helpers::filler_test()]),
            allowed_tests: json!([]),
            expected_messages: &[
                "SQBRTEST202 models/orders.sql: model \"orders\" has 0 tests; 1 required (1 empty-input-only test not counted; see SQBRTEST203)",
            ],
        },
        test_types::EmptyInputMinimumTestsTestCase {
            description: "the same test with input rows satisfies the minimum",
            select: json!(["SQBRTEST202"]),
            tests: json!([real_test]),
            allowed_tests: json!([]),
            expected_messages: &[],
        },
    ];

    for test_case in test_cases {
        let messages = helpers::empty_input_rule_evaluation(
            &project_dir,
            test_case.select,
            test_case.tests,
            test_case.allowed_tests,
            true,
        )?;
        assert_eq!(
            messages, test_case.expected_messages,
            "{}",
            test_case.description
        );
    }
    Ok(())
}

#[test]
fn given_sql_server_conversions_when_evaluating_empty_input_rule_then_reports_the_dialect_unavailable()
-> Result<(), String> {
    let project_dir = TempDir::new().map_err(|error| error.to_string())?;
    let empty_orders = (
        "__source__raw_orders",
        "SELECT NULL AS order_id WHERE 1 = 0",
    );
    let test_cases = [
        test_types::EmptyInputTestRuleTestCase {
            description: "a CONVERT to a parameterized type under a false predicate is an empty mock, reported unavailable until SQLBuild parses the dialect",
            test: helpers::empty_input_test_fact(
                "orders__empty_inputs_produce_no_rows",
                &[(
                    "__source__raw_orders",
                    "SELECT CONVERT(DECIMAL(18, 2), NULL) AS amount WHERE 1 = 0",
                )],
                &[],
                &[NO_ROWS_ASSERTION],
            ),
            allowed_tests: json!([]),
            expected_messages: &[UNAVAILABLE_MESSAGE],
        },
        test_types::EmptyInputTestRuleTestCase {
            description: "a CONVERT with a style argument is an empty mock, reported unavailable until SQLBuild parses the dialect",
            test: helpers::empty_input_test_fact(
                "orders__empty_inputs_produce_no_rows",
                &[(
                    "__source__raw_orders",
                    "SELECT CONVERT(NUMERIC(10, 2), NULL, 1) AS amount WHERE 1 = 0",
                )],
                &[],
                &[NO_ROWS_ASSERTION],
            ),
            allowed_tests: json!([]),
            expected_messages: &[UNAVAILABLE_MESSAGE],
        },
        test_types::EmptyInputTestRuleTestCase {
            description: "a TRY_CONVERT to a parameterized type keeps a bare existence check, reported unavailable until SQLBuild parses the dialect",
            test: helpers::empty_input_test_fact(
                "orders__empty_inputs_produce_no_rows",
                &[empty_orders],
                &[],
                &[(
                    "__assert__empty_inputs_produce_no_rows",
                    "SELECT TRY_CONVERT(VARCHAR(10), order_id) AS unexpected_row FROM __ref(\"orders\")",
                )],
            ),
            allowed_tests: json!([]),
            expected_messages: &[UNAVAILABLE_MESSAGE],
        },
        test_types::EmptyInputTestRuleTestCase {
            description: "a function call in a CONVERT value argument still counts, reported unavailable until SQLBuild parses the dialect",
            test: helpers::empty_input_test_fact(
                "orders__empty_inputs_produce_no_rows",
                &[(
                    "__source__raw_orders",
                    "SELECT CONVERT(VARCHAR(10), COUNT(*)) AS amount WHERE 1 = 0",
                )],
                &[],
                &[NO_ROWS_ASSERTION],
            ),
            allowed_tests: json!([]),
            expected_messages: &[UNAVAILABLE_MESSAGE],
        },
    ];

    for test_case in test_cases {
        let messages = helpers::dialect_empty_input_rule_evaluation(
            "mssql",
            &project_dir,
            json!(["SQBRTEST203"]),
            json!([test_case.test]),
            test_case.allowed_tests,
            false,
        )?;
        assert_eq!(
            messages, test_case.expected_messages,
            "{}",
            test_case.description
        );
    }
    Ok(())
}
