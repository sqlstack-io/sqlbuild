use crate::analysis_route::main::analysis_route::analysis_route;
use crate::analysis_route::main::native_gap::native_gap;
use crate::analysis_route::main::skipped_analysis::skipped_analysis;
use crate::analysis_route::models::{AnalysisRoute, LocatedGap, ParserGap};
use crate::analysis_route::tests::test_types::NativeGapTestCase;
use crate::analysis_route::types::NativeDialect;
use crate::assembly::project::models::SyntaxCheck;

/// A test dialect whose parser refuses the word PIVOT and an unfilled `@@@` placeholder.
struct PivotlessDialect;

impl NativeDialect for PivotlessDialect {
    fn name(&self) -> &'static str {
        "postgres"
    }

    fn read_whole(&self, sql: &str) -> Result<(), ParserGap> {
        let refused = ["PIVOT", "@@@"]
            .iter()
            .filter_map(|word| sql.find(word).map(|offset| (offset, *word)))
            .min();
        match refused {
            Some((offset, word)) => Err(ParserGap {
                construct: word.to_owned(),
                offset,
            }),
            None => Ok(()),
        }
    }
}

#[test]
fn given_any_dialect_when_routing_analysis_then_it_stays_on_the_legacy_path() {
    for dialect in [
        Some("postgres"),
        Some("snowflake"),
        Some("duckdb"),
        Some("sqlite"),
        Some("bigquery"),
        Some("databricks"),
        Some("clickhouse"),
        Some("tsql"),
        None,
    ] {
        assert!(
            matches!(analysis_route(dialect), AnalysisRoute::Legacy),
            "{dialect:?}"
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
    let gap = LocatedGap {
        construct: "PIVOT".to_owned(),
        line: 3,
        column: 3,
    };
    assert_eq!(
        skipped_analysis("orders_pivot", "postgres", &gap),
        (
            "P026",
            "SQL analysis skipped for model 'orders_pivot': the postgres parser cannot read PIVOT \
             at line 3, column 3"
                .to_owned()
        )
    );
}
