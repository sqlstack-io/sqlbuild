use crate::analysis_route::main::analysis_route::analysis_route;
use crate::analysis_route::main::native_gap::native_gap;
use crate::analysis_route::main::skipped_analysis::skipped_analysis;
use crate::analysis_route::models::{AnalysisRoute, BigQueryDialect, LocatedGap};
use crate::analysis_route::tests::test_types::{
    NativeGapTestCase, NormalizeTypeTestCase, NormalizedTypeTestCase, ParseFailureTestCase,
    PivotlessDialect, RouteTestCase, SkippedAnalysisTestCase,
};
use crate::analysis_route::types::NativeDialect;
use crate::assembly::project::models::SyntaxCheck;
use crate::type_system::models::{NormalizedType, PythonInteger, TypeFamily};

#[test]
fn given_a_dialect_when_routing_analysis_then_only_native_dialects_leave_the_legacy_path() {
    let test_cases = [
        RouteTestCase {
            description: "postgres",
            dialect: Some("postgres"),
            expected_legacy: true,
        },
        RouteTestCase {
            description: "snowflake",
            dialect: Some("snowflake"),
            expected_legacy: true,
        },
        RouteTestCase {
            description: "duckdb",
            dialect: Some("duckdb"),
            expected_legacy: true,
        },
        RouteTestCase {
            description: "sqlite",
            dialect: Some("sqlite"),
            expected_legacy: true,
        },
        RouteTestCase {
            description: "bigquery",
            dialect: Some("bigquery"),
            expected_legacy: false,
        },
        RouteTestCase {
            description: "databricks",
            dialect: Some("databricks"),
            expected_legacy: true,
        },
        RouteTestCase {
            description: "clickhouse",
            dialect: Some("clickhouse"),
            expected_legacy: true,
        },
        RouteTestCase {
            description: "tsql",
            dialect: Some("tsql"),
            expected_legacy: true,
        },
        RouteTestCase {
            description: "no analysis dialect",
            dialect: None,
            expected_legacy: true,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            matches!(analysis_route(test_case.dialect), AnalysisRoute::Legacy),
            test_case.expected_legacy,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_model_sql_when_probing_the_native_parser_then_its_gap_is_located_after_placeholders() {
    let test_cases = [
        NativeGapTestCase {
            description: "SQL the parser reads whole has no gap",
            sql: "SELECT 1 AS a",
            placeholders: &[],
            expected_gap: None,
        },
        NativeGapTestCase {
            description: "a placeholder is filled before the parser reads the SQL",
            sql: "SELECT * FROM t WHERE region = @@@region",
            placeholders: &[("region", "'eu'")],
            expected_gap: None,
        },
        NativeGapTestCase {
            description: "a UDF call's stub is spelled as the dialect accepts it",
            sql: "SELECT __udf(\"scaled_amount\")(amount, 2) AS scaled FROM t",
            placeholders: &[],
            expected_gap: None,
        },
        NativeGapTestCase {
            description: "cursor bounds are stubs spelled as the dialect accepts them",
            sql: "SELECT * FROM t WHERE at >= __cursor_start() AND at < __cursor_end( )",
            placeholders: &[],
            expected_gap: None,
        },
        NativeGapTestCase {
            description: "a quoted hook parameter becomes the string literal it is filled with",
            sql: "SELECT @'relation' AS refreshed_relation, @'label' AS refresh_label",
            placeholders: &[],
            expected_gap: None,
        },
        NativeGapTestCase {
            description: "the gap's line and column count from 1, in characters",
            sql: "SELECT 'é' AS a\nFROM t\n  PIVOT (sum(x) FOR y IN (1))",
            placeholders: &[],
            expected_gap: Some(LocatedGap {
                construct: "PIVOT".to_owned(),
                line: 3,
                column: 3,
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
            native_gap(&PivotlessDialect, &check),
            Ok(test_case.expected_gap),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_bigquery_model_sql_when_probing_its_parser_then_only_sql_it_reads_whole_has_no_gap() {
    let test_cases = [
        NativeGapTestCase {
            description: "a query BigQuery reads whole",
            sql: "SELECT 1 AS a FROM t WHERE region = @@@region",
            placeholders: &[("region", "'eu'")],
            expected_gap: None,
        },
        NativeGapTestCase {
            description: "a UDF call's stub reads as one name",
            sql: "SELECT __udf(\"scaled_amount\")(amount, 2) AS scaled FROM t",
            placeholders: &[],
            expected_gap: None,
        },
        NativeGapTestCase {
            description: "cursor bounds read as stub calls",
            sql: "SELECT * FROM t WHERE updated_at >= __cursor_start() AND updated_at < __cursor_end( )",
            placeholders: &[],
            expected_gap: None,
        },
        NativeGapTestCase {
            description: "a syntax error is located where GoogleSQL locates it",
            sql: "SELECT 1 FROM FROM",
            placeholders: &[],
            expected_gap: Some(LocatedGap {
                construct: "Syntax error at or near 'FROM'".to_owned(),
                line: 1,
                column: 15,
            }),
        },
        NativeGapTestCase {
            description: "a scripting statement is a named gap",
            sql: "DECLARE x INT64 DEFAULT 1",
            placeholders: &[],
            expected_gap: Some(LocatedGap {
                construct: "declare".to_owned(),
                line: 1,
                column: 1,
            }),
        },
        NativeGapTestCase {
            description: "a second statement is not read whole",
            sql: "SELECT 1;\nSELECT 2",
            placeholders: &[],
            expected_gap: Some(LocatedGap {
                construct: "more than one statement".to_owned(),
                line: 2,
                column: 9,
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
            native_gap(&BigQueryDialect, &check),
            Ok(test_case.expected_gap),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_located_gap_when_skipping_analysis_then_p026_names_the_model_and_the_gap() {
    let test_cases = [SkippedAnalysisTestCase {
        description: "a PIVOT gap on line 3",
        model: "orders_pivot",
        dialect: "postgres",
        gap: LocatedGap {
            construct: "PIVOT".to_owned(),
            line: 3,
            column: 3,
        },
        expected_code: "P026",
        expected_message: "SQL analysis skipped for model 'orders_pivot': the postgres parser \
                           cannot read PIVOT at line 3, column 3",
    }];
    for test_case in test_cases {
        assert_eq!(
            skipped_analysis(test_case.model, test_case.dialect, &test_case.gap),
            (
                test_case.expected_code,
                test_case.expected_message.to_owned()
            ),
            "{}",
            test_case.description
        );
    }
}

/// Validity follows a BigQuery dry run of `SELECT CAST(NULL AS <spelling>)`; names stay Polyglot's.
#[test]
fn given_a_bigquery_type_spelling_when_normalising_then_a_valid_one_keeps_polyglot_s_name() {
    let test_cases = [
        NormalizeTypeTestCase {
            description: "lower-case INT64",
            spelling: "int64",
            expected_name: Some("INT64"),
        },
        NormalizeTypeTestCase {
            description: "BYTEINT keeps Polyglot's spelling",
            spelling: "BYTEINT",
            expected_name: Some("BYTEINT"),
        },
        NormalizeTypeTestCase {
            description: "TINYINT keeps Polyglot's spelling",
            spelling: "TINYINT",
            expected_name: Some("TINYINT"),
        },
        NormalizeTypeTestCase {
            description: "SMALLINT is INT64",
            spelling: "SMALLINT",
            expected_name: Some("INT64"),
        },
        NormalizeTypeTestCase {
            description: "DECIMAL with parameters is NUMERIC",
            spelling: "DECIMAL(38, 9)",
            expected_name: Some("NUMERIC(38,9)"),
        },
        NormalizeTypeTestCase {
            description: "the widest NUMERIC precision",
            spelling: "NUMERIC(29)",
            expected_name: Some("NUMERIC(29)"),
        },
        NormalizeTypeTestCase {
            description: "NUMERIC precision past 29 is refused",
            spelling: "NUMERIC(30)",
            expected_name: None,
        },
        NormalizeTypeTestCase {
            description: "NUMERIC scale past 9 is refused",
            spelling: "NUMERIC(10,10)",
            expected_name: None,
        },
        NormalizeTypeTestCase {
            description: "a whole BIGDECIMAL is Polyglot's NUMERIC",
            spelling: "BIGDECIMAL",
            expected_name: Some("NUMERIC"),
        },
        NormalizeTypeTestCase {
            description: "BIGNUMERIC precision past 38 without a scale is refused",
            spelling: "BIGNUMERIC(40)",
            expected_name: None,
        },
        NormalizeTypeTestCase {
            description: "the widest BIGNUMERIC",
            spelling: "BIGNUMERIC(76, 38)",
            expected_name: Some("BIGNUMERIC(76,38)"),
        },
        NormalizeTypeTestCase {
            description: "BIGNUMERIC precision past 76 is refused",
            spelling: "BIGNUMERIC(77,38)",
            expected_name: None,
        },
        NormalizeTypeTestCase {
            description: "BOOL is BOOLEAN",
            spelling: "BOOL",
            expected_name: Some("BOOLEAN"),
        },
        NormalizeTypeTestCase {
            description: "a whole TIMESTAMP is Polyglot's DATETIME",
            spelling: "TIMESTAMP",
            expected_name: Some("DATETIME"),
        },
        NormalizeTypeTestCase {
            description: "DATETIME",
            spelling: "DATETIME",
            expected_name: Some("DATETIME"),
        },
        NormalizeTypeTestCase {
            description: "a padded STRING length",
            spelling: "STRING( 10 )",
            expected_name: Some("STRING(10)"),
        },
        NormalizeTypeTestCase {
            description: "a zero STRING length is refused",
            spelling: "STRING(0)",
            expected_name: None,
        },
        NormalizeTypeTestCase {
            description: "nested field names are upper case",
            spelling: "array<struct<x int64, y string>>",
            expected_name: Some("ARRAY<STRUCT<XINT64,YSTRING>>"),
        },
        NormalizeTypeTestCase {
            description: "BIGINT keeps its spelling inside ARRAY",
            spelling: "ARRAY<BIGINT>",
            expected_name: Some("ARRAY<BIGINT>"),
        },
        NormalizeTypeTestCase {
            description: "TIMESTAMP inside ARRAY is DATETIME",
            spelling: "ARRAY<TIMESTAMP>",
            expected_name: Some("ARRAY<DATETIME>"),
        },
        NormalizeTypeTestCase {
            description: "STRUCT renames INTEGER but keeps BIGDECIMAL",
            spelling: "STRUCT<int INTEGER, `a b` BIGDECIMAL>",
            expected_name: Some("STRUCT<INTINT64,`AB`BIGDECIMAL>"),
        },
        NormalizeTypeTestCase {
            description: "an unnamed field with parameters keeps its spelling",
            spelling: "STRUCT<DECIMAL(38,9), DECIMAL>",
            expected_name: Some("STRUCT<DECIMAL(38,9),NUMERIC>"),
        },
        NormalizeTypeTestCase {
            description: "a whole STRING drops MAX",
            spelling: "STRING(MAX)",
            expected_name: Some("STRING"),
        },
        NormalizeTypeTestCase {
            description: "a whole BIGDECIMAL drops MAX and keeps its scale",
            spelling: "BIGDECIMAL(MAX, 2)",
            expected_name: Some("NUMERIC(2)"),
        },
        NormalizeTypeTestCase {
            description: "BYTES keeps MAX",
            spelling: "BYTES(MAX)",
            expected_name: Some("BYTES(MAX)"),
        },
        NormalizeTypeTestCase {
            description: "a nested STRING keeps MAX",
            spelling: "ARRAY<STRING(MAX)>",
            expected_name: Some("ARRAY<STRING(MAX)>"),
        },
        NormalizeTypeTestCase {
            description: "NUMERIC takes no MAX",
            spelling: "NUMERIC(MAX)",
            expected_name: None,
        },
        NormalizeTypeTestCase {
            description: "a RANGE of DATE",
            spelling: "range<date>",
            expected_name: Some("RANGE<DATE>"),
        },
        NormalizeTypeTestCase {
            description: "a RANGE of INT64 is refused",
            spelling: "RANGE<INT64>",
            expected_name: None,
        },
        NormalizeTypeTestCase {
            description: "TIMESTAMP takes no precision",
            spelling: "TIMESTAMP(6)",
            expected_name: None,
        },
        NormalizeTypeTestCase {
            description: "FLOAT is no BigQuery type",
            spelling: "FLOAT",
            expected_name: None,
        },
        NormalizeTypeTestCase {
            description: "VARCHAR is no BigQuery type",
            spelling: "VARCHAR(10)",
            expected_name: None,
        },
        NormalizeTypeTestCase {
            description: "a collated STRING is no cast target",
            spelling: "STRING COLLATE 'und:ci'",
            expected_name: None,
        },
        NormalizeTypeTestCase {
            description: "a spelling that closes the cast early",
            spelling: "INT64) FROM t WHERE (1",
            expected_name: None,
        },
        NormalizeTypeTestCase {
            description: "a spelling that comments out the rest",
            spelling: "INT64) -- ",
            expected_name: None,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            BigQueryDialect
                .normalize_type(test_case.spelling)
                .map(|normalized| normalized.normalized_name)
                .as_deref(),
            test_case.expected_name,
            "{}",
            test_case.description
        );
    }
}

/// Each expected type is the one Polyglot's `normalize_type(spelling, "bigquery")` gives.
#[test]
fn given_a_valid_bigquery_type_spelling_when_normalising_then_the_type_is_polyglot_s() {
    let test_cases = [
        NormalizedTypeTestCase {
            description: "TIMESTAMP is Polyglot's DATETIME of the timestamp family",
            spelling: "TIMESTAMP",
            expected_type: ("DATETIME", TypeFamily::Timestamp, [None, None, None]),
        },
        NormalizedTypeTestCase {
            description: "BOOL is Polyglot's BOOLEAN",
            spelling: "BOOL",
            expected_type: ("BOOLEAN", TypeFamily::Boolean, [None, None, None]),
        },
        NormalizedTypeTestCase {
            description: "TINYINT keeps its name in the integer family",
            spelling: "TINYINT",
            expected_type: ("TINYINT", TypeFamily::Integer, [None, None, None]),
        },
        NormalizedTypeTestCase {
            description: "a NUMERIC alias keeps its precision and scale",
            spelling: "DECIMAL(38, 9)",
            expected_type: (
                "NUMERIC(38,9)",
                TypeFamily::Decimal,
                [Some(38), Some(9), None],
            ),
        },
        NormalizedTypeTestCase {
            description: "a sized STRING keeps its length",
            spelling: "STRING( 10 )",
            expected_type: ("STRING(10)", TypeFamily::String, [None, None, Some(10)]),
        },
        NormalizedTypeTestCase {
            description: "BYTES(MAX) is of no family Polyglot knows",
            spelling: "BYTES(MAX)",
            expected_type: ("BYTES(MAX)", TypeFamily::Other, [None, None, None]),
        },
        NormalizedTypeTestCase {
            description: "a top-level RANGE is read as text, so of the timestamp family",
            spelling: "range<timestamp>",
            expected_type: (
                "RANGE<TIMESTAMP>",
                TypeFamily::Timestamp,
                [None, None, None],
            ),
        },
        NormalizedTypeTestCase {
            description: "an ARRAY is of no family",
            spelling: "ARRAY<STRUCT<a TIMESTAMP>>",
            expected_type: (
                "ARRAY<STRUCT<ADATETIME>>",
                TypeFamily::Other,
                [None, None, None],
            ),
        },
        NormalizedTypeTestCase {
            description: "a RANGE closed by >> fails Polyglot's parse, so is read as text",
            spelling: "ARRAY<RANGE<TIMESTAMP>>",
            expected_type: (
                "ARRAY<RANGE<TIMESTAMP>>",
                TypeFamily::Timestamp,
                [None, None, None],
            ),
        },
        NormalizedTypeTestCase {
            description: "a RANGE closed by > > parses",
            spelling: "ARRAY< RANGE< TIMESTAMP > >",
            expected_type: (
                "ARRAY<RANGE<TIMESTAMP>>",
                TypeFamily::Other,
                [None, None, None],
            ),
        },
        NormalizedTypeTestCase {
            description: "a named RANGE field parses",
            spelling: "STRUCT<a RANGE<TIMESTAMP> >",
            expected_type: (
                "STRUCT<ARANGE<TIMESTAMP>>",
                TypeFamily::Other,
                [None, None, None],
            ),
        },
        NormalizedTypeTestCase {
            description: "an unnamed RANGE field fails Polyglot's parse",
            spelling: "STRUCT< RANGE< TIMESTAMP > >",
            expected_type: (
                "STRUCT<RANGE<TIMESTAMP>>",
                TypeFamily::Timestamp,
                [None, None, None],
            ),
        },
    ];
    let normalized =
        |(name, family, [precision, scale, length]): (&str, TypeFamily, [Option<i64>; 3])| {
            NormalizedType {
                normalized_name: name.to_owned(),
                family,
                precision: precision.map(PythonInteger::from),
                scale: scale.map(PythonInteger::from),
                length: length.map(PythonInteger::from),
            }
        };
    for test_case in test_cases {
        assert_eq!(
            BigQueryDialect.normalize_type(test_case.spelling),
            Some(normalized(test_case.expected_type)),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_sql_bigquery_refuses_when_parsing_then_the_gap_says_whether_it_is_a_syntax_error() {
    let test_cases = [
        ParseFailureTestCase {
            description: "a syntax error, at its byte offset after a multi-byte character",
            sql: "SELECT 'é' FROM FROM".to_owned(),
            expected_syntax: true,
            expected_offset: Some(17),
        },
        ParseFailureTestCase {
            description: "the nesting limit is not a syntax error",
            sql: format!("SELECT {}1{}", "(".repeat(300), ")".repeat(300)),
            expected_syntax: false,
            expected_offset: None,
        },
    ];
    for test_case in test_cases {
        let gap = BigQueryDialect
            .parse(&test_case.sql)
            .expect_err(test_case.description);
        assert_eq!(
            (gap.syntax, test_case.expected_offset.map(|_| gap.offset)),
            (test_case.expected_syntax, test_case.expected_offset),
            "{}",
            test_case.description
        );
    }
}
