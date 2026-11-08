use crate::analysis_route::models::{PostgresDialect, TypeMatch};
use crate::analysis_route::tests::test_types::NativeTypeMatchTestCase;
use crate::analysis_route::types::NativeDialect;

#[test]
fn given_postgres_type_spellings_when_matching_types_then_the_types_postgres_names_are_compared() {
    let compared = |same_type: bool| TypeMatch::Compared {
        same_type,
        nullable_into_non_null: false,
    };
    let test_cases = [
        NativeTypeMatchTestCase {
            description: "INT and INTEGER both name integer",
            declared: "INTEGER",
            inferred: Some("INT"),
            expected_match: compared(true),
        },
        NativeTypeMatchTestCase {
            description: "int4 is integer",
            declared: "int4",
            inferred: Some("integer"),
            expected_match: compared(true),
        },
        NativeTypeMatchTestCase {
            description: "VARCHAR(10) is character varying(10)",
            declared: "character varying(10)",
            inferred: Some("VARCHAR(10)"),
            expected_match: compared(true),
        },
        NativeTypeMatchTestCase {
            description: "a different length is a different type",
            declared: "VARCHAR(10)",
            inferred: Some("VARCHAR(20)"),
            expected_match: compared(false),
        },
        NativeTypeMatchTestCase {
            description: "TIMESTAMP is timestamp without time zone",
            declared: "timestamp without time zone",
            inferred: Some("TIMESTAMP"),
            expected_match: compared(true),
        },
        NativeTypeMatchTestCase {
            description: "TIMESTAMPTZ is not TIMESTAMP",
            declared: "TIMESTAMPTZ",
            inferred: Some("TIMESTAMP"),
            expected_match: compared(false),
        },
        NativeTypeMatchTestCase {
            description: "float8 is double precision",
            declared: "DOUBLE PRECISION",
            inferred: Some("float8"),
            expected_match: compared(true),
        },
        NativeTypeMatchTestCase {
            description: "integer and bigint differ",
            declared: "INTEGER",
            inferred: Some("BIGINT"),
            expected_match: compared(false),
        },
        NativeTypeMatchTestCase {
            description: "a type PostgreSQL's grammar refuses is not compared",
            declared: "SETOF integer",
            inferred: Some("integer"),
            expected_match: TypeMatch::NotCompared {
                nullable_into_non_null: false,
            },
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            PostgresDialect.type_match(test_case.declared, test_case.inferred, true),
            test_case.expected_match,
            "{}",
            test_case.description
        );
    }
}
