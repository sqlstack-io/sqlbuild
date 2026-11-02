use crate::type_system::main::normalize_type::normalize_type;
use crate::type_system::main::same_type::same_type;
use crate::type_system::tests::test_types::SameTypeTestCase;

#[test]
fn given_two_types_when_comparing_then_an_unstated_snowflake_temporal_precision_matches() {
    let test_cases = [
        SameTypeTestCase {
            description: "a declared millisecond time matches metadata's precision-less time",
            dialect: "snowflake",
            types: ("TIME(3)", "TIME"),
            expected_same: true,
        },
        SameTypeTestCase {
            description: "a declared nanosecond time matches metadata's precision-less time",
            dialect: "snowflake",
            types: ("TIME", "TIME(9)"),
            expected_same: true,
        },
        SameTypeTestCase {
            description: "two stated time precisions must agree",
            dialect: "snowflake",
            types: ("TIME(3)", "TIME(9)"),
            expected_same: false,
        },
        SameTypeTestCase {
            description: "a declared offset timestamp precision matches metadata's spelling",
            dialect: "snowflake",
            types: ("TIMESTAMP_TZ(9)", "TIMESTAMP_TZ"),
            expected_same: true,
        },
        SameTypeTestCase {
            description: "two stated offset timestamp precisions must agree",
            dialect: "snowflake",
            types: ("TIMESTAMP_TZ(3)", "TIMESTAMP_TZ(9)"),
            expected_same: false,
        },
        SameTypeTestCase {
            description: "local time zone timestamps with and without precision",
            dialect: "snowflake",
            types: ("TIMESTAMP_LTZ(6)", "TIMESTAMP_LTZ"),
            expected_same: true,
        },
        SameTypeTestCase {
            description: "an offset timestamp is not a local time zone timestamp",
            dialect: "snowflake",
            types: ("TIMESTAMP_TZ", "TIMESTAMP_LTZ(9)"),
            expected_same: false,
        },
        SameTypeTestCase {
            description: "time is not a timestamp",
            dialect: "snowflake",
            types: ("TIME(9)", "TIMESTAMP_NTZ"),
            expected_same: false,
        },
        SameTypeTestCase {
            description: "other dialects keep exact comparison",
            dialect: "duckdb",
            types: ("TIME(3)", "TIME"),
            expected_same: false,
        },
        SameTypeTestCase {
            description: "an unstated text length is still a different type",
            dialect: "snowflake",
            types: ("VARCHAR(10)", "VARCHAR(20)"),
            expected_same: false,
        },
    ];
    for test_case in test_cases {
        let (left, right) = test_case.types;
        let left = normalize_type(left, test_case.dialect)
            .expect("the type normalizes")
            .normalized;
        let right = normalize_type(right, test_case.dialect)
            .expect("the type normalizes")
            .normalized;
        assert_eq!(
            same_type(&left, &right, test_case.dialect),
            test_case.expected_same,
            "{}",
            test_case.description
        );
    }
}
