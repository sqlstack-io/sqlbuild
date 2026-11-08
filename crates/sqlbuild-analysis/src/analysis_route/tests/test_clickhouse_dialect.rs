use crate::analysis_route::constants::CLICKHOUSE_DIALECT;
use crate::analysis_route::main::native_gap::native_gap;
use crate::analysis_route::models::{ClickHouseDialect, LocatedGap, TypeMatch};
use crate::analysis_route::tests::test_types::{
    NativeGapTestCase, TypeMatchTestCase, TypeSpellingsTestCase,
};
use crate::analysis_route::types::NativeDialect;
use crate::assembly::project::models::SyntaxCheck;
use crate::type_system::main::normalize_type::normalize_type;

/// Type spellings from SQLBuild's compiled ClickHouse corpus, with case, nesting and width variants.
const CLICKHOUSE_SPELLINGS: &[&str] = &[
    "Array(INT(11))",
    "Array(Int32)",
    "Array(String)",
    "BIGINT",
    "BIGINT(20)",
    "BIGINT(20) UNSIGNED",
    "BINARY",
    "BINARY(5)",
    "BOOLEAN",
    "Bool",
    "CHAR",
    "DATE",
    "DECIMAL(10,2)",
    "DOUBLE",
    "DOUBLE PRECISION",
    "DOUBLE(10,2)",
    "Date32",
    "DateTime",
    "DateTime('UTC')",
    "DateTime64( 6 , 'Europe/Amsterdam' )",
    "DateTime64(3)",
    "DateTime64(3, 'UTC')",
    "DateTime64(3,'UTC')",
    "Decimal",
    "Decimal(10, 2)",
    "Decimal(38, 4)",
    "Decimal32(2)",
    "FLOAT",
    "FLOAT(12)",
    "FLOAT(53)",
    "FixedString",
    "FixedString(3)",
    "FixedString(4)",
    "Float32",
    "Float64",
    "Float64(3)",
    "INT UNSIGNED",
    "INT(11)",
    "INTEGER",
    "INTEGER(11)",
    "IPv4",
    "Int128",
    "Int16",
    "Int32",
    "Int32(11)",
    "Int64",
    "Int8",
    "JSON",
    "LowCardinality(Nullable(String))",
    "LowCardinality(String)",
    "Map(String, Int64)",
    "NUMERIC(10)",
    "NUMERIC(12, 3)",
    "Nothing",
    "Nullable(DateTime64(3,'UTC'))",
    "Nullable(Int32)",
    "Nullable(Int64)",
    "Nullable(String)",
    "REAL",
    "SMALLINT",
    "SMALLINT(5)",
    "String",
    "TEXT",
    "TIMESTAMP",
    "TINYINT",
    "TINYINT(1)",
    "UInt16",
    "UInt256",
    "UInt32",
    "UInt64",
    "UInt8",
    "UUID",
    "VARCHAR",
    "VARCHAR(10)",
    "VARCHAR(22)",
    "int",
    "Float",
    "double",
    "timestamp",
    "varchar(5)",
    "boolean",
    "Array(Nullable(INT))",
    "Tuple(a INT)",
    "Enum8('a' = 1)",
    "Nullable(INT)",
    "LowCardinality(VARCHAR)",
    "REAL(5)",
    "TIMESTAMP(3)",
    "DECIMAL(10)",
    "numeric",
    "Map(String, BIGINT)",
    "Array(Array(TEXT))",
    "Nullable(TIMESTAMP)",
    "Array(VARCHAR(10))",
    "Array(BOOLEAN)",
    "LowCardinality(INT)",
    "Nullable(Nullable(INT))",
    "Nullable(BOOLEAN)",
    "Tuple(INT, VARCHAR)",
];

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
            description: "an apostrophe in a hash comment opens no string",
            sql: "WITH o AS (SELECT 1 AS id)\n# don't keep the star\nSELECT * FROM o",
            placeholders: &[],
            expected_gap: None,
        },
        NativeGapTestCase {
            description: "an apostrophe in a shebang comment opens no string",
            sql: "SELECT * FROM t #!it's\n",
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
            description: "WITH TIES is a named gap at its line and column",
            sql: "SELECT 'é' AS a\nFROM t ORDER BY a\n  LIMIT 1 WITH TIES",
            placeholders: &[],
            expected_gap: Some(LocatedGap {
                construct: "with_ties".to_owned(),
                message: "with_ties".to_owned(),
                line: 3,
                column: 11,
                syntax: false,
            }),
        },
        NativeGapTestCase {
            description: "invalid SQL is a gap at the parser's refusal",
            sql: "SELECT (1 + FROM t",
            placeholders: &[],
            expected_gap: Some(LocatedGap {
                construct: "Syntax error: failed at position 18 (t)".to_owned(),
                message: "Syntax error: failed at position 18 (t)".to_owned(),
                line: 1,
                column: 18,
                syntax: true,
            }),
        },
        NativeGapTestCase {
            description: "two statements are not one model body",
            sql: "SELECT 1; SELECT 2",
            placeholders: &[],
            expected_gap: Some(LocatedGap {
                construct: "more than one statement".to_owned(),
                message: "more than one statement".to_owned(),
                line: 1,
                column: 1,
                syntax: false,
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
fn given_a_clickhouse_type_when_normalising_then_it_is_polyglots_type_family_included() {
    let test_cases = [TypeSpellingsTestCase {
        description: "every corpus spelling and its case, nesting and width variants",
        spellings: CLICKHOUSE_SPELLINGS,
        expected_mismatches: &[],
    }];
    for test_case in test_cases {
        let mismatches: Vec<&str> = test_case
            .spellings
            .iter()
            .copied()
            .filter(|spelling| {
                let polyglot = normalize_type(spelling, CLICKHOUSE_DIALECT)
                    .ok()
                    .map(|typed| typed.normalized);
                ClickHouseDialect.normalize_type(spelling) != polyglot
            })
            .collect();
        assert_eq!(
            mismatches, test_case.expected_mismatches,
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
            inferred: Some("Int32"),
            inferred_nullable: false,
            expected_match: TypeMatch::Compared {
                same_type: true,
                nullable_into_non_null: false,
            },
        },
        TypeMatchTestCase {
            description: "a declared BIGINT is the engine's Int64",
            declared: "BIGINT",
            inferred: Some("Int64"),
            inferred_nullable: false,
            expected_match: TypeMatch::Compared {
                same_type: true,
                nullable_into_non_null: false,
            },
        },
        TypeMatchTestCase {
            description: "a declared VARCHAR refuses a nullable String",
            declared: "VARCHAR",
            inferred: Some("String"),
            inferred_nullable: true,
            expected_match: TypeMatch::Compared {
                same_type: true,
                nullable_into_non_null: true,
            },
        },
        TypeMatchTestCase {
            description: "argument spacing does not change the type",
            declared: "DateTime64(3,'UTC')",
            inferred: Some("DateTime64(3, 'UTC')"),
            inferred_nullable: false,
            expected_match: TypeMatch::Compared {
                same_type: true,
                nullable_into_non_null: false,
            },
        },
        TypeMatchTestCase {
            description: "a different width is a different type",
            declared: "BIGINT",
            inferred: Some("Int32"),
            inferred_nullable: false,
            expected_match: TypeMatch::Compared {
                same_type: false,
                nullable_into_non_null: false,
            },
        },
        TypeMatchTestCase {
            description: "a declared BIGINT refuses a nullable value of unknown type",
            declared: "BIGINT",
            inferred: None,
            inferred_nullable: true,
            expected_match: TypeMatch::NotCompared {
                nullable_into_non_null: true,
            },
        },
        TypeMatchTestCase {
            description: "a declared Nullable type takes a nullable value of unknown type",
            declared: "Nullable(BIGINT)",
            inferred: None,
            inferred_nullable: true,
            expected_match: TypeMatch::NotCompared {
                nullable_into_non_null: false,
            },
        },
        TypeMatchTestCase {
            description: "a type ClickHouse refuses is not compared",
            declared: "FixedString",
            inferred: Some("String"),
            inferred_nullable: true,
            expected_match: TypeMatch::NotCompared {
                nullable_into_non_null: false,
            },
        },
        TypeMatchTestCase {
            description: "a misspelt inferred type is not compared, and a nullable one still meets a non-null declaration",
            declared: "String",
            inferred: Some("Strng"),
            inferred_nullable: true,
            expected_match: TypeMatch::NotCompared {
                nullable_into_non_null: true,
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
