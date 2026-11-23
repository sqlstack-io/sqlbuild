use std::collections::HashSet;

use crate::functions::main::table_function_partition_error::table_function_partition_error;
use crate::functions::tests::test_types::TableFunctionPartitionTestCase;

#[test]
fn given_table_function_calls_when_checking_partitions_then_first_unpartitioned_call_is_reported() {
    let whole_partition: HashSet<String> = HashSet::from(["smooth_readings".to_owned()]);
    let test_cases = [
        TableFunctionPartitionTestCase {
            description: "a partitioned call and other functions' calls pass",
            calls: vec![
                ("table_fn", "smooth_readings", true),
                ("table_fn", "order_window", false),
                ("ref", "readings", false),
            ],
            expected_error: None,
        },
        TableFunctionPartitionTestCase {
            description: "an unpartitioned call names the clause to add",
            calls: vec![
                ("table_fn", "order_window", false),
                ("table_fn", "smooth_readings", false),
            ],
            expected_error: Some(
                "Model file sql/models/report.sql calls Python table function 'smooth_readings' \
                 without OVER (PARTITION BY ...); it receives one whole partition per call, so \
                 add OVER (PARTITION BY <columns>) after the call",
            ),
        },
        TableFunctionPartitionTestCase {
            description: "a udf call of the same name is not a table function call",
            calls: vec![("udf", "smooth_readings", false)],
            expected_error: None,
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            table_function_partition_error(
                "Model file sql/models/report.sql",
                &test_case.calls,
                &whole_partition
            )
            .as_deref(),
            test_case.expected_error,
            "{}",
            test_case.description
        );
    }
}
