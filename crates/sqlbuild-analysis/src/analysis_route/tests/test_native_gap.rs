use crate::analysis_route::main::analysis_route::analysis_route;
use crate::analysis_route::main::native_gap::native_gap;
use crate::analysis_route::main::skipped_analysis::skipped_analysis;
use crate::analysis_route::models::{AnalysisRoute, GapPosition, GapSite, LocatedGap};
use crate::analysis_route::tests::test_types::{
    NativeGapTestCase, PivotlessDialect, RouteTestCase, SkippedAnalysisTestCase,
};
use crate::assembly::project::models::SyntaxCheck;
use crate::compiled_project::models::TextPosition;

#[test]
fn given_a_dialect_when_routing_analysis_then_only_native_dialects_leave_the_legacy_path() {
    let test_cases = [
        RouteTestCase {
            description: "postgres",
            dialect: Some("postgres"),
            expected_legacy: false,
        },
        RouteTestCase {
            description: "snowflake",
            dialect: Some("snowflake"),
            expected_legacy: false,
        },
        RouteTestCase {
            description: "duckdb",
            dialect: Some("duckdb"),
            expected_legacy: false,
        },
        RouteTestCase {
            description: "sqlite",
            dialect: Some("sqlite"),
            expected_legacy: false,
        },
        RouteTestCase {
            description: "bigquery",
            dialect: Some("bigquery"),
            expected_legacy: true,
        },
        RouteTestCase {
            description: "databricks",
            dialect: Some("databricks"),
            expected_legacy: true,
        },
        RouteTestCase {
            description: "clickhouse",
            dialect: Some("clickhouse"),
            expected_legacy: false,
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
                message: "PIVOT".to_owned(),
                line: 3,
                column: 3,
                syntax: false,
                source_offset: Some(25),
            }),
        },
        NativeGapTestCase {
            description: "a stub respelt before the gap keeps its offset in the checked SQL",
            sql: "SELECT __cursor_start() AS a FROM t PIVOT (sum(x) FOR y IN (1))",
            placeholders: &[],
            expected_gap: Some(LocatedGap {
                construct: "PIVOT".to_owned(),
                message: "PIVOT".to_owned(),
                line: 1,
                column: 46,
                syntax: false,
                source_offset: Some(36),
            }),
        },
        NativeGapTestCase {
            description: "text changed on both sides of the gap leaves it only in the analysed SQL",
            sql: "SELECT __cursor_start() AS a FROM t PIVOT (sum(x) FOR y IN (@@@n))",
            placeholders: &[("n", "1")],
            expected_gap: Some(LocatedGap {
                construct: "PIVOT".to_owned(),
                message: "PIVOT".to_owned(),
                line: 1,
                column: 46,
                syntax: false,
                source_offset: None,
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
fn given_a_located_gap_when_skipping_analysis_then_p026_names_the_model_its_site_and_the_gap() {
    let test_cases = [
        SkippedAnalysisTestCase {
            description: "a query gap placed in the model file, with the parser's message",
            model: "orders",
            dialect: "postgres",
            gap: LocatedGap {
                construct: "syntax error".to_owned(),
                message: "syntax error at or near \"FROM\"".to_owned(),
                line: 1,
                column: 15,
                syntax: true,
                source_offset: Some(14),
            },
            site: GapSite {
                label: None,
                path: "models/orders.sql",
                position: GapPosition::File(TextPosition {
                    line: 5,
                    column: 18,
                }),
            },
            expected_code: "P026",
            expected_message: "SQL analysis skipped for model 'orders' (models/orders.sql): the \
                               postgres parser cannot read syntax error at line 5, column 18: \
                               syntax error at or near \"FROM\"",
        },
        SkippedAnalysisTestCase {
            description: "a hook gap names the hook and is placed in its statement",
            model: "customer_totals",
            dialect: "duckdb",
            gap: LocatedGap {
                construct: "Syntax error at or near 'WHERE'".to_owned(),
                message: "Syntax error at or near 'WHERE'".to_owned(),
                line: 1,
                column: 13,
                syntax: true,
                source_offset: Some(12),
            },
            site: GapSite {
                label: Some("post_hooks[0] sql(\"record_refresh\")"),
                path: "hooks/sql/record_refresh.sql",
                position: GapPosition::Statement(TextPosition {
                    line: 1,
                    column: 13,
                }),
            },
            expected_code: "P026",
            expected_message: "SQL analysis skipped for model 'customer_totals' post_hooks[0] \
                               sql(\"record_refresh\") (hooks/sql/record_refresh.sql): the duckdb \
                               parser cannot read Syntax error at or near 'WHERE' at line 1, \
                               column 13 of its SQL",
        },
        SkippedAnalysisTestCase {
            description: "a gap only the analysed SQL places says so",
            model: "orders_pivot",
            dialect: "postgres",
            gap: LocatedGap {
                construct: "PIVOT".to_owned(),
                message: "PIVOT".to_owned(),
                line: 3,
                column: 3,
                syntax: false,
                source_offset: None,
            },
            site: GapSite {
                label: None,
                path: "models/orders_pivot.sql",
                position: GapPosition::Analysed,
            },
            expected_code: "P026",
            expected_message: "SQL analysis skipped for model 'orders_pivot' \
                               (models/orders_pivot.sql): the postgres parser cannot read PIVOT at \
                               line 3, column 3 of the analysed SQL",
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            skipped_analysis(
                test_case.model,
                test_case.dialect,
                &test_case.gap,
                &test_case.site
            ),
            (
                test_case.expected_code,
                test_case.expected_message.to_owned()
            ),
            "{}",
            test_case.description
        );
    }
}
