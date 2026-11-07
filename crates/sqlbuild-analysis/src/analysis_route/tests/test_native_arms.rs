use crate::analysis_route::main::native_gap::native_gap;
use crate::analysis_route::models::{LocatedGap, SnowflakeDialect, SqliteDialect};
use crate::analysis_route::tests::test_types::ArmGapTestCase;
use crate::assembly::project::models::SyntaxCheck;

#[test]
fn given_model_sql_when_probing_a_dialects_parser_then_only_sql_it_reads_whole_has_no_gap() {
    let test_cases = [
        ArmGapTestCase {
            dialect: &SqliteDialect,
            description: "a query SQLite reads whole",
            sql: "SELECT 1 AS a FROM t WHERE region = @@@region",
            placeholders: &[("region", "'eu'")],
            expected_gap: None,
        },
        ArmGapTestCase {
            dialect: &SqliteDialect,
            description: "a UDF call's stub reads as one unquoted name",
            sql: "SELECT __udf(\"scaled_amount\")(amount, 2) AS scaled FROM t",
            placeholders: &[],
            expected_gap: None,
        },
        ArmGapTestCase {
            dialect: &SqliteDialect,
            description: "cursor bounds read as stub calls",
            sql: "SELECT * FROM t WHERE at >= __cursor_start() AND at < __cursor_end( )",
            placeholders: &[],
            expected_gap: None,
        },
        ArmGapTestCase {
            dialect: &SqliteDialect,
            description: "a syntax error is located where the parser stops",
            sql: "SELECT 1 FROM FROM",
            placeholders: &[],
            expected_gap: Some(LocatedGap {
                construct: "near \"FROM\": syntax error".to_owned(),
                line: 1,
                column: 15,
            }),
        },
        ArmGapTestCase {
            dialect: &SqliteDialect,
            description: "PIVOT, which SQLite reads as an alias, fails at its parenthesis as sqlite3 does",
            sql: "SELECT a\nFROM t\n  PIVOT (sum(x) FOR y IN (1))",
            placeholders: &[],
            expected_gap: Some(LocatedGap {
                construct: "near \"(\": syntax error".to_owned(),
                line: 3,
                column: 9,
            }),
        },
        ArmGapTestCase {
            dialect: &SqliteDialect,
            description: "a second statement is not read whole",
            sql: "SELECT 1;\nSELECT 2",
            placeholders: &[],
            expected_gap: Some(LocatedGap {
                construct: "more than one statement".to_owned(),
                line: 2,
                column: 9,
            }),
        },
        ArmGapTestCase {
            dialect: &SnowflakeDialect,
            description: "a QUALIFY query Snowflake reads whole",
            sql: "SELECT a FROM t QUALIFY row_number() OVER (PARTITION BY a ORDER BY b) = 1",
            placeholders: &[],
            expected_gap: None,
        },
        ArmGapTestCase {
            dialect: &SnowflakeDialect,
            description: "a UDF call's stub reads as one unquoted name",
            sql: "SELECT __udf(\"scaled_amount\")(amount, 2) AS scaled FROM t",
            placeholders: &[],
            expected_gap: None,
        },
        ArmGapTestCase {
            dialect: &SnowflakeDialect,
            description: "cursor bounds read as stub calls",
            sql: "SELECT * FROM t WHERE at >= __cursor_start() AND at < __cursor_end( )",
            placeholders: &[],
            expected_gap: None,
        },
        ArmGapTestCase {
            dialect: &SnowflakeDialect,
            description: "a syntax error is located where the parser stops",
            sql: "SELECT 1 FROM FROM",
            placeholders: &[],
            expected_gap: Some(LocatedGap {
                construct: "Expected table name or subquery, got From".to_owned(),
                line: 1,
                column: 15,
            }),
        },
        ArmGapTestCase {
            dialect: &SnowflakeDialect,
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
            native_gap(test_case.dialect, &check),
            Ok(test_case.expected_gap),
            "{}: {}",
            test_case.dialect.name(),
            test_case.description
        );
    }
}
