use crate::analysis_route::main::clickhouse_type_match::clickhouse_type_match;
use crate::analysis_route::models::TypeMatch;
use crate::analysis_route::tests::test_types::TypeMatchTestCase;

#[test]
fn given_clickhouse_column_types_when_matching_then_wrappers_are_stripped_and_nullability_compared_apart()
 {
    let test_cases = [
        TypeMatchTestCase {
            description: "a nullable String flag matches DESCRIBE's Nullable(String)",
            declared: "Nullable(String)",
            inferred: "String",
            inferred_nullable: true,
            expected_match: TypeMatch::Compared {
                same_type: true,
                nullable_into_non_null: false,
            },
        },
        TypeMatchTestCase {
            description: "a declared LowCardinality(String) is a String",
            declared: "LowCardinality(String)",
            inferred: "String",
            inferred_nullable: false,
            expected_match: TypeMatch::Compared {
                same_type: true,
                nullable_into_non_null: false,
            },
        },
        TypeMatchTestCase {
            description: "LowCardinality around Nullable allows null",
            declared: "LowCardinality(Nullable(String))",
            inferred: "Nullable(String)",
            inferred_nullable: false,
            expected_match: TypeMatch::Compared {
                same_type: true,
                nullable_into_non_null: false,
            },
        },
        TypeMatchTestCase {
            description: "a nullable inference into a plain String declaration is a nullability conflict",
            declared: "String",
            inferred: "String",
            inferred_nullable: true,
            expected_match: TypeMatch::Compared {
                same_type: true,
                nullable_into_non_null: true,
            },
        },
        TypeMatchTestCase {
            description: "a non-null inference into a Nullable declaration is no conflict",
            declared: "Nullable(Int64)",
            inferred: "Int64",
            inferred_nullable: false,
            expected_match: TypeMatch::Compared {
                same_type: true,
                nullable_into_non_null: false,
            },
        },
        TypeMatchTestCase {
            description: "different base types differ whatever their wrappers",
            declared: "Nullable(Int32)",
            inferred: "Int64",
            inferred_nullable: true,
            expected_match: TypeMatch::Compared {
                same_type: false,
                nullable_into_non_null: false,
            },
        },
        TypeMatchTestCase {
            description: "a Nullable nested inside Array is part of the type, not stripped",
            declared: "Array(Nullable(String))",
            inferred: "Array(String)",
            inferred_nullable: false,
            expected_match: TypeMatch::Compared {
                same_type: false,
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
