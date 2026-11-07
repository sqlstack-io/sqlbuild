use crate::analysis_route::main::analysis_route::analysis_route;
use crate::analysis_route::main::native_gap::native_gap;
use crate::analysis_route::main::skipped_analysis::skipped_analysis;
use crate::analysis_route::models::{AnalysisRoute, BigQueryDialect, LocatedGap};
use crate::analysis_route::tests::test_types::{
    NativeGapTestCase, NormalizeTypeTestCase, ParseFailureTestCase, PivotlessDialect,
    RouteTestCase, SkippedAnalysisTestCase,
};
use crate::analysis_route::types::NativeDialect;
use crate::assembly::project::models::SyntaxCheck;

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

/// Each expectation is the type a BigQuery dry run of `SELECT CAST(NULL AS <spelling>)` gave,
/// or its refusal, in SQLBuild's comparison spelling.
#[test]
fn given_a_bigquery_type_spelling_when_normalising_then_it_names_the_type_bigquery_gives_it() {
    let test_cases = [
        NormalizeTypeTestCase {
            spelling: "int64",
            expected: Some("INT64"),
        },
        NormalizeTypeTestCase {
            spelling: "BYTEINT",
            expected: Some("INT64"),
        },
        NormalizeTypeTestCase {
            spelling: "TINYINT",
            expected: Some("INT64"),
        },
        NormalizeTypeTestCase {
            spelling: "DECIMAL(38, 9)",
            expected: Some("NUMERIC(38,9)"),
        },
        NormalizeTypeTestCase {
            spelling: "NUMERIC(29)",
            expected: Some("NUMERIC(29)"),
        },
        NormalizeTypeTestCase {
            spelling: "NUMERIC(30)",
            expected: None,
        },
        NormalizeTypeTestCase {
            spelling: "NUMERIC(10,10)",
            expected: None,
        },
        NormalizeTypeTestCase {
            spelling: "BIGDECIMAL",
            expected: Some("BIGNUMERIC"),
        },
        NormalizeTypeTestCase {
            spelling: "BIGNUMERIC(40)",
            expected: None,
        },
        NormalizeTypeTestCase {
            spelling: "BIGNUMERIC(76, 38)",
            expected: Some("BIGNUMERIC(76,38)"),
        },
        NormalizeTypeTestCase {
            spelling: "BIGNUMERIC(77,38)",
            expected: None,
        },
        NormalizeTypeTestCase {
            spelling: "BOOL",
            expected: Some("BOOLEAN"),
        },
        NormalizeTypeTestCase {
            spelling: "TIMESTAMP",
            expected: Some("TIMESTAMP"),
        },
        NormalizeTypeTestCase {
            spelling: "DATETIME",
            expected: Some("DATETIME"),
        },
        NormalizeTypeTestCase {
            spelling: "STRING( 10 )",
            expected: Some("STRING(10)"),
        },
        NormalizeTypeTestCase {
            spelling: "STRING(0)",
            expected: None,
        },
        NormalizeTypeTestCase {
            spelling: "array<struct<x int64, y string>>",
            expected: Some("ARRAY<STRUCT<XINT64,YSTRING>>"),
        },
        NormalizeTypeTestCase {
            spelling: "STRUCT<INT64, STRING>",
            expected: Some("STRUCT<INT64,STRING>"),
        },
        NormalizeTypeTestCase {
            spelling: "range<date>",
            expected: Some("RANGE<DATE>"),
        },
        NormalizeTypeTestCase {
            spelling: "RANGE<INT64>",
            expected: None,
        },
        NormalizeTypeTestCase {
            spelling: "TIMESTAMP(6)",
            expected: None,
        },
        NormalizeTypeTestCase {
            spelling: "FLOAT",
            expected: None,
        },
        NormalizeTypeTestCase {
            spelling: "VARCHAR(10)",
            expected: None,
        },
        NormalizeTypeTestCase {
            spelling: "STRING COLLATE 'und:ci'",
            expected: None,
        },
        NormalizeTypeTestCase {
            spelling: "INT64) FROM t WHERE (1",
            expected: None,
        },
        NormalizeTypeTestCase {
            spelling: "INT64) -- ",
            expected: None,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            BigQueryDialect
                .normalize_type(test_case.spelling)
                .as_deref(),
            test_case.expected,
            "{}",
            test_case.spelling
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
            gap.syntax, test_case.expected_syntax,
            "{}",
            test_case.description
        );
        if let Some(offset) = test_case.expected_offset {
            assert_eq!(gap.offset, offset, "{}", test_case.description);
        }
    }
}
