use crate::analysis_route::main::analysis_route::analysis_route;
use crate::analysis_route::main::native_gap::native_gap;
use crate::analysis_route::main::skipped_analysis::skipped_analysis;
use crate::analysis_route::models::{AnalysisRoute, BigQueryDialect, LocatedGap};
use crate::analysis_route::tests::test_types::{
    NativeGapTestCase, PivotlessDialect, RouteTestCase, SkippedAnalysisTestCase,
};
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
