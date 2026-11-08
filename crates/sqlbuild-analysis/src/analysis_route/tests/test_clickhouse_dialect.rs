use crate::analysis_route::main::native_gap::native_gap;
use crate::analysis_route::models::{ClickHouseDialect, LocatedGap, TypeMatch};
use crate::analysis_route::tests::test_types::{
    ClickHouseSpellingTestCase, NativeGapTestCase, TypeMatchTestCase,
};
use crate::assembly::project::models::SyntaxCheck;
use crate::type_system::_helpers::dialects::coerce_type_dialect;
use crate::type_system::_helpers::fallback::normalize_with_fallback;
use crate::type_system::models::TypeFamily;

#[test]
fn given_clickhouse_model_sql_when_probing_the_native_parser_then_only_unreadable_sql_has_a_gap() {
    let test_cases = [
        NativeGapTestCase {
            description: "a UDF call's stub is one unquoted name",
            sql: "SELECT __udf(\"scaled_amount\")(amount, 2) AS scaled FROM t",
            placeholders: &[],
            expected_gap: None,
        },
        NativeGapTestCase {
            description: "cursor bounds are stubs ClickHouse reads",
            sql: "SELECT * FROM t WHERE at >= __cursor_start() AND at < __cursor_end( )",
            placeholders: &[],
            expected_gap: None,
        },
        NativeGapTestCase {
            description: "a quoted hook parameter becomes a string literal",
            sql: "SELECT * FROM t WHERE region = @'region'",
            placeholders: &[],
            expected_gap: None,
        },
        NativeGapTestCase {
            description: "a FROM after FROM is a table name, as ClickHouse reads it",
            sql: "SELECT 1 FROM FROM",
            placeholders: &[],
            expected_gap: None,
        },
        NativeGapTestCase {
            description: "INTO OUTFILE is a named gap at its line and column",
            sql: "SELECT 'é' AS a\nFROM t\n  INTO OUTFILE 'x.csv'",
            placeholders: &[],
            expected_gap: Some(LocatedGap {
                construct: "into_outfile".to_owned(),
                line: 3,
                column: 3,
            }),
        },
        NativeGapTestCase {
            description: "two statements are not one model body",
            sql: "SELECT 1; SELECT 2",
            placeholders: &[],
            expected_gap: Some(LocatedGap {
                construct: "more than one statement".to_owned(),
                line: 1,
                column: 19,
            }),
        },
    ];
    for test_case in test_cases {
        let check = SyntaxCheck {
            sql: test_case.sql.to_owned(),
            placeholders: test_case
                .placeholders
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect(),
        };
        assert_eq!(
            native_gap(&ClickHouseDialect, &check),
            Ok(test_case.expected_gap),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_invalid_clickhouse_sql_when_probing_the_native_parser_then_it_has_a_gap() {
    let check = SyntaxCheck {
        sql: "SELECT (1 + FROM t".to_owned(),
        placeholders: Default::default(),
    };
    let gap = native_gap(&ClickHouseDialect, &check);
    assert!(
        matches!(gap, Ok(Some(LocatedGap { line: 1, .. }))),
        "{gap:?}"
    );
}

#[test]
fn given_a_clickhouse_type_when_normalising_then_the_fallback_gives_polyglots_canonical_name() {
    let test_cases = [
        ClickHouseSpellingTestCase {
            description: "VARCHAR is Polyglot's STRING",
            spelling: "VARCHAR(22)",
            expected_name: "STRING",
            expected_family: TypeFamily::String,
        },
        ClickHouseSpellingTestCase {
            description: "BOOLEAN is Polyglot's BOOL",
            spelling: "boolean",
            expected_name: "BOOL",
            expected_family: TypeFamily::Boolean,
        },
        ClickHouseSpellingTestCase {
            description: "NUMERIC keeps its precision as DECIMAL",
            spelling: "NUMERIC(10)",
            expected_name: "DECIMAL(10)",
            expected_family: TypeFamily::Decimal,
        },
        ClickHouseSpellingTestCase {
            description: "an alias inside an array is renamed",
            spelling: "Array(INT(11))",
            expected_name: "ARRAY(NULLABLE(INT32))",
            expected_family: TypeFamily::Other,
        },
        ClickHouseSpellingTestCase {
            description: "an alias Polyglot makes nullable is not wrapped twice",
            spelling: "Nullable(INT)",
            expected_name: "NULLABLE(INT32)",
            expected_family: TypeFamily::Other,
        },
        ClickHouseSpellingTestCase {
            description: "LowCardinality keeps its argument as written",
            spelling: "LowCardinality(VARCHAR)",
            expected_name: "LOWCARDINALITY(VARCHAR)",
            expected_family: TypeFamily::Other,
        },
        ClickHouseSpellingTestCase {
            description: "an engine type keeps its spacing-free name",
            spelling: "DateTime64(3, 'UTC')",
            expected_name: "DATETIME64(3,'UTC')",
            expected_family: TypeFamily::Other,
        },
        ClickHouseSpellingTestCase {
            description: "INTEGER gets Polyglot's name; Polyglot's family is Integer, the fallback's Other",
            spelling: "INTEGER",
            expected_name: "NULLABLE(INT32)",
            expected_family: TypeFamily::Other,
        },
    ];
    for test_case in test_cases {
        let normalized = ClickHouseDialect
            .normalize_type(test_case.spelling)
            .map(|spelling| normalize_with_fallback(&spelling, coerce_type_dialect("clickhouse")));
        assert_eq!(
            normalized.map(|normalized| (normalized.normalized_name, normalized.family)),
            Some((
                test_case.expected_name.to_owned(),
                test_case.expected_family
            )),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_clickhouse_contract_types_when_matching_then_aliases_compare_as_the_engine_spells_them() {
    let test_cases = [
        TypeMatchTestCase {
            description: "a declared INTEGER is a non-null Int32",
            declared: "INTEGER",
            inferred: "Int32",
            inferred_nullable: false,
            expected_match: TypeMatch {
                same_type: true,
                nullable_into_non_null: false,
            },
        },
        TypeMatchTestCase {
            description: "a declared VARCHAR refuses a nullable String",
            declared: "VARCHAR",
            inferred: "String",
            inferred_nullable: true,
            expected_match: TypeMatch {
                same_type: true,
                nullable_into_non_null: true,
            },
        },
        TypeMatchTestCase {
            description: "argument spacing does not change the type",
            declared: "DateTime64(3,'UTC')",
            inferred: "DateTime64(3, 'UTC')",
            inferred_nullable: false,
            expected_match: TypeMatch {
                same_type: true,
                nullable_into_non_null: false,
            },
        },
        TypeMatchTestCase {
            description: "a different width is a different type",
            declared: "BIGINT",
            inferred: "Int32",
            inferred_nullable: false,
            expected_match: TypeMatch {
                same_type: false,
                nullable_into_non_null: false,
            },
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            ClickHouseDialect.type_match(
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
