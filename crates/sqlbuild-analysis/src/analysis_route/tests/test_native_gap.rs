use crate::analysis_route::main::analysis_route::analysis_route;
use crate::analysis_route::main::native_gap::native_gap;
use crate::analysis_route::main::skipped_analysis::skipped_analysis;
use crate::analysis_route::models::{AnalysisRoute, LocatedGap, TsqlDialect};
use crate::analysis_route::tests::test_types::{
    NativeGapTestCase, PivotlessDialect, RouteTestCase, SkippedAnalysisTestCase,
};
use crate::analysis_route::types::NativeDialect;
use crate::assembly::project::models::SyntaxCheck;
use crate::type_system::models::TypeFamily;

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
            expected_legacy: true,
        },
        RouteTestCase {
            description: "tsql",
            dialect: Some("tsql"),
            expected_legacy: false,
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

#[test]
fn given_tsql_model_sql_when_probing_its_parser_then_only_sql_it_reads_whole_has_no_gap() {
    let test_cases = [
        NativeGapTestCase {
            description: "a query SQL Server reads whole, placeholder filled",
            sql: "SELECT TOP (5) [id] AS a FROM dbo.t WHERE region = @@@region",
            placeholders: &[("region", "N'eu'")],
            expected_gap: None,
        },
        NativeGapTestCase {
            description: "a UDF call's stub reads as a schema-qualified call",
            sql: "SELECT __udf(\"scaled_amount\")(amount, 2) AS scaled FROM t",
            placeholders: &[],
            expected_gap: None,
        },
        NativeGapTestCase {
            description: "cursor bounds read as schema-qualified stub calls",
            sql: "SELECT * FROM t WHERE updated_at >= __cursor_start() AND updated_at < __cursor_end( )",
            placeholders: &[],
            expected_gap: None,
        },
        NativeGapTestCase {
            description: "an unknown one-part function stops the parser at its call, where SQL Server raises 195",
            sql: "SELECT id,\n  LENGTH(name) AS n FROM t",
            placeholders: &[],
            expected_gap: Some(LocatedGap {
                construct: "not a recognized built-in function name".to_owned(),
                line: 2,
                column: 9,
            }),
        },
        NativeGapTestCase {
            description: "LIMIT reads as a table alias, so SQL Server stops at its count",
            sql: "SELECT id FROM t LIMIT 5",
            placeholders: &[],
            expected_gap: Some(LocatedGap {
                construct: "Incorrect syntax near '5'.".to_owned(),
                line: 1,
                column: 24,
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
            native_gap(&TsqlDialect, &check),
            Ok(test_case.expected_gap),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_tsql_parser_errors_when_parsing_then_syntax_errors_are_told_from_gaps() {
    let refused = TsqlDialect.parse("SELECT LENGTH(name) FROM t");
    assert_eq!(
        refused.map_err(|gap| (gap.syntax, gap.offset)),
        Err((true, 13))
    );
    let parsed = TsqlDialect
        .parse("SELECT 1; SELECT 2")
        .expect("two statements parse");
    assert_eq!((parsed.statements.len(), parsed.gaps.len()), (2, 0));
}

#[test]
fn given_tsql_type_spellings_when_normalizing_then_they_take_polyglots_canonical_names() {
    // Polyglot's T-SQL names, measured on the compiled corpus's spellings; `None` where
    // neither SQL Server 16.0.4305.5 nor SQLBuild's logical types know the type.
    let test_cases = [
        ("int", Some(("INTEGER", TypeFamily::Integer))),
        (
            "numeric(12, 2)",
            Some(("DECIMAL(12,2)", TypeFamily::Decimal)),
        ),
        ("dec", Some(("DEC", TypeFamily::Other))),
        ("FLOAT(24)", Some(("FLOAT(24)", TypeFamily::Float))),
        ("DOUBLE PRECISION", Some(("FLOAT", TypeFamily::Float))),
        ("DOUBLE", Some(("FLOAT", TypeFamily::Float))),
        ("BOOLEAN", Some(("BIT", TypeFamily::Boolean))),
        ("TIMESTAMP", Some(("DATETIME2", TypeFamily::Timestamp))),
        ("NTEXT", Some(("VARCHAR(MAX)", TypeFamily::String))),
        (
            "national character varying(10)",
            Some(("VARCHAR(10)", TypeFamily::String)),
        ),
        ("nvarchar(max)", Some(("NVARCHAR(MAX)", TypeFamily::Other))),
        ("datetime2(3)", Some(("DATETIME2(3)", TypeFamily::Other))),
        ("rowversion", Some(("ROWVERSION", TypeFamily::Other))),
        ("dbo.money_code", None),
        ("INT64", None),
    ];
    for (spelling, expected) in test_cases {
        let normalized = TsqlDialect.normalize_type(spelling);
        assert_eq!(
            normalized
                .as_ref()
                .map(|normalized| (normalized.normalized_name.as_str(), normalized.family)),
            expected,
            "{spelling}"
        );
    }
}
