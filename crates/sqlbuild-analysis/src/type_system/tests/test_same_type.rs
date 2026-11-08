use std::collections::BTreeMap;

use crate::type_system::main::normalize_type::normalize_type;
use crate::type_system::main::same_type::same_type;
use crate::type_system::tests::helpers::normalized_type;
use crate::type_system::tests::test_types::{AliasGroupsTestCase, SameTypeTestCase};

/// Each dialect's groups of valid spellings that the Polyglot-based normalization read alike.
const ALIAS_GROUPS: &str = include_str!("alias_groups.json");

#[test]
fn given_two_types_when_comparing_then_same_type_answers() {
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
        SameTypeTestCase {
            description: "Postgres FLOAT without a precision is DOUBLE PRECISION, not REAL",
            dialect: "postgres",
            types: ("FLOAT", "REAL"),
            expected_same: false,
        },
        SameTypeTestCase {
            description: "BigQuery's TIMESTAMP is an absolute time, not a civil DATETIME",
            dialect: "bigquery",
            types: ("TIMESTAMP", "TIMESTAMP WITHOUT TIME ZONE"),
            expected_same: false,
        },
        SameTypeTestCase {
            description: "BigQuery's civil time is DATETIME",
            dialect: "bigquery",
            types: ("DATETIME", "TIMESTAMP WITHOUT TIME ZONE"),
            expected_same: true,
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

#[test]
fn given_spellings_of_one_type_when_comparing_then_every_pair_is_the_same_type() {
    let test_cases = [AliasGroupsTestCase {
        description: "every alias group of the Polyglot-based normalization",
        groups: ALIAS_GROUPS,
        expected_same: true,
    }];
    for test_case in test_cases {
        let groups: BTreeMap<String, Vec<Vec<String>>> =
            serde_json::from_str(test_case.groups).expect("the alias groups are JSON");
        for (dialect, dialect_groups) in &groups {
            for group in dialect_groups {
                for (index, left) in group.iter().enumerate() {
                    for right in &group[index + 1..] {
                        assert_eq!(
                            same_type(
                                &normalized_type(left, dialect),
                                &normalized_type(right, dialect),
                                dialect,
                            ),
                            test_case.expected_same,
                            "{}: {dialect} `{left}` and `{right}`",
                            test_case.description
                        );
                    }
                }
            }
        }
    }
}
