use crate::analysis_route::_helpers::dialects::clickhouse_types::clickhouse_type_match;
use crate::analysis_route::models::TypeMatch;
use crate::analysis_route::tests::test_types::TypeMatchTestCase;

#[test]
fn given_clickhouse_column_types_when_matching_then_wrappers_are_stripped_and_nullability_compared_apart()
 {
    let test_cases = [
        TypeMatchTestCase {
            description: "a nullable String flag matches DESCRIBE's Nullable(String)",
            declared: "Nullable(String)",
            inferred: Some("String"),
            inferred_nullable: true,
            expected_match: TypeMatch::Compared {
                same_type: true,
                nullable_into_non_null: false,
            },
        },
        TypeMatchTestCase {
            description: "a declared LowCardinality(String) is a String",
            declared: "LowCardinality(String)",
            inferred: Some("String"),
            inferred_nullable: false,
            expected_match: TypeMatch::Compared {
                same_type: true,
                nullable_into_non_null: false,
            },
        },
        TypeMatchTestCase {
            description: "LowCardinality around Nullable allows null",
            declared: "LowCardinality(Nullable(String))",
            inferred: Some("Nullable(String)"),
            inferred_nullable: false,
            expected_match: TypeMatch::Compared {
                same_type: true,
                nullable_into_non_null: false,
            },
        },
        TypeMatchTestCase {
            description: "a nullable inference into a plain String declaration is a nullability conflict",
            declared: "String",
            inferred: Some("String"),
            inferred_nullable: true,
            expected_match: TypeMatch::Compared {
                same_type: true,
                nullable_into_non_null: true,
            },
        },
        TypeMatchTestCase {
            description: "a non-null inference into a Nullable declaration is no conflict",
            declared: "Nullable(Int64)",
            inferred: Some("Int64"),
            inferred_nullable: false,
            expected_match: TypeMatch::Compared {
                same_type: true,
                nullable_into_non_null: false,
            },
        },
        TypeMatchTestCase {
            description: "different base types differ whatever their wrappers",
            declared: "Nullable(Int32)",
            inferred: Some("Int64"),
            inferred_nullable: true,
            expected_match: TypeMatch::Compared {
                same_type: false,
                nullable_into_non_null: false,
            },
        },
        TypeMatchTestCase {
            description: "a Nullable nested inside Array is part of the type, not stripped",
            declared: "Array(Nullable(String))",
            inferred: Some("Array(String)"),
            inferred_nullable: false,
            expected_match: TypeMatch::Compared {
                same_type: false,
                nullable_into_non_null: false,
            },
        },
        TypeMatchTestCase {
            description: "spacing inside a type's arguments is not a difference",
            declared: "Decimal(10, 2)",
            inferred: Some("Decimal(10,2)"),
            inferred_nullable: false,
            expected_match: TypeMatch::Compared {
                same_type: true,
                nullable_into_non_null: false,
            },
        },
        TypeMatchTestCase {
            description: "spacing inside a quoted enum value is part of the type",
            declared: "Enum8('a b' = 1)",
            inferred: Some("Enum8('ab' = 1)"),
            inferred_nullable: false,
            expected_match: TypeMatch::Compared {
                same_type: false,
                nullable_into_non_null: false,
            },
        },
        TypeMatchTestCase {
            description: "a nullable value of unknown type into a plain String is a nullability conflict",
            declared: "String",
            inferred: None,
            inferred_nullable: true,
            expected_match: TypeMatch::NotCompared {
                nullable_into_non_null: true,
            },
        },
        TypeMatchTestCase {
            description: "a nullable value of unknown type into a Nullable column is no conflict",
            declared: "Nullable(String)",
            inferred: None,
            inferred_nullable: true,
            expected_match: TypeMatch::NotCompared {
                nullable_into_non_null: false,
            },
        },
        TypeMatchTestCase {
            description: "a non-null value of unknown type is no conflict",
            declared: "String",
            inferred: None,
            inferred_nullable: false,
            expected_match: TypeMatch::NotCompared {
                nullable_into_non_null: false,
            },
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            clickhouse_type_match(
                test_case.declared,
                test_case.inferred,
                test_case.inferred_nullable
            ),
            test_case.expected_match,
            "{}",
            test_case.description
        );
    }
}
