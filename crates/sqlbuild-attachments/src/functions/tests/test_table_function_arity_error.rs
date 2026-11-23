use std::collections::HashMap;

use crate::functions::main::table_function_arity_error::table_function_arity_error;
use crate::functions::tests::test_types::TableFunctionArityTestCase;

#[test]
fn given_table_function_calls_when_checking_arity_then_first_mismatch_is_reported() {
    let counts: HashMap<String, usize> = HashMap::from([
        ("customer_orders".to_owned(), 1),
        ("order_window".to_owned(), 2),
    ]);
    let test_cases = [
        TableFunctionArityTestCase {
            description: "matching counts and other reference kinds pass",
            calls: vec![
                ("table_fn", "customer_orders", Some(1)),
                ("ref", "orders", None),
            ],
            expected_error: None,
        },
        TableFunctionArityTestCase {
            description: "one declared argument reads in the singular",
            calls: vec![("table_fn", "customer_orders", Some(2))],
            expected_error: Some(
                "Model file sql/models/report.sql table function 'customer_orders' expects 1 \
                 argument but received 2",
            ),
        },
        TableFunctionArityTestCase {
            description: "an unknown count reads as Python's None",
            calls: vec![("table_fn", "order_window", None)],
            expected_error: Some(
                "Model file sql/models/report.sql table function 'order_window' expects 2 \
                 arguments but received None",
            ),
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            table_function_arity_error("Model file sql/models/report.sql", &test_case.calls, &counts)
                .as_deref(),
            test_case.expected_error,
            "{}",
            test_case.description
        );
    }
}
