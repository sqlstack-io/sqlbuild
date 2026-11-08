use crate::analysis_route::main::unapplied_nullability_rules::unapplied_nullability_rules;
use crate::analysis_route::main::unavailable_analysis::unavailable_analysis;
use crate::analysis_route::models::{
    AnalysisRoute, DuckDbDialect, NativeAnalysis, SnowflakeDialect, SqliteDialect, TypeMatch,
};
use crate::analysis_route::tests::helpers::{analysed, resource_columns};
use crate::analysis_route::tests::test_types::{
    CataloguedDialect, ColumnTestCase, FindingTestCase, FoldingDialect, LineageTestCase,
    MacroBodyTestCase, NativeTypeMatchTestCase, PivotlessDialect, ReadWholeTestCase, StarTestCase,
    UnappliedRulesTestCase, UnavailableAnalysisTestCase,
};
use crate::analysis_route::types::NativeDialect;

#[test]
fn given_sql_when_reading_it_whole_then_only_one_parsed_statement_without_a_gap_is_read() {
    let test_cases = [
        ReadWholeTestCase {
            description: "one query is read whole",
            sql: "SELECT a FROM t",
            expected_gap: None,
        },
        ReadWholeTestCase {
            description: "two statements are not one model",
            sql: "SELECT 1; SELECT 2",
            expected_gap: Some("more than one statement"),
        },
        ReadWholeTestCase {
            description: "no statement at all",
            sql: "  ",
            expected_gap: Some("no statement"),
        },
        ReadWholeTestCase {
            description: "a comment alone holds no statement, not several",
            sql: "-- nothing to read",
            expected_gap: Some("no statement"),
        },
        ReadWholeTestCase {
            description: "a statement kept as raw text is not read",
            sql: "RAW VACUUM t",
            expected_gap: Some("unparsed statement"),
        },
        ReadWholeTestCase {
            description: "a construct the parser refuses stops it first",
            sql: "SELECT * FROM t PIVOT (sum(x) FOR y IN (1))",
            expected_gap: Some("PIVOT"),
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            PivotlessDialect
                .read_whole(test_case.sql)
                .err()
                .map(|gap| gap.construct),
            test_case.expected_gap.map(str::to_owned),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_type_the_dialect_refuses_when_matching_types_then_nothing_is_compared() {
    let test_cases = [
        NativeTypeMatchTestCase {
            description: "a refused declared type is not compared",
            declared: "NOT_A_TYPE",
            inferred: Some("INTEGER"),
            expected_match: TypeMatch::NotCompared {
                nullable_into_non_null: false,
            },
        },
        NativeTypeMatchTestCase {
            description: "a refused inferred type is not compared",
            declared: "INTEGER",
            inferred: Some("NOT_A_TYPE"),
            expected_match: TypeMatch::NotCompared {
                nullable_into_non_null: false,
            },
        },
        NativeTypeMatchTestCase {
            description: "an unknown inferred type is not compared",
            declared: "INTEGER",
            inferred: None,
            expected_match: TypeMatch::NotCompared {
                nullable_into_non_null: false,
            },
        },
        NativeTypeMatchTestCase {
            description: "two known, different types still mismatch",
            declared: "INTEGER",
            inferred: Some("TEXT"),
            expected_match: TypeMatch::Compared {
                same_type: false,
                nullable_into_non_null: false,
            },
        },
        NativeTypeMatchTestCase {
            description: "the same type matches",
            declared: "INTEGER",
            inferred: Some("integer"),
            expected_match: TypeMatch::Compared {
                same_type: true,
                nullable_into_non_null: false,
            },
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            PivotlessDialect.type_match(test_case.declared, test_case.inferred, true),
            test_case.expected_match,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_no_native_parser_when_skipping_analysis_then_p026_names_the_dialect() {
    let test_cases = [
        UnavailableAnalysisTestCase {
            description: "an unknown dialect is named",
            dialect: Some("trino"),
            expected_message: "SQL analysis skipped for model 'orders': no native parser reads \
                               the trino dialect",
        },
        UnavailableAnalysisTestCase {
            description: "no dialect is the generic dialect",
            dialect: None,
            expected_message: "SQL analysis skipped for model 'orders': no native parser reads \
                               the generic dialect",
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            unavailable_analysis("orders", test_case.dialect),
            ("P026", test_case.expected_message.to_owned()),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_adapter_nullability_rules_when_routing_then_only_a_native_route_warns_they_are_unapplied()
{
    let test_cases = [
        UnappliedRulesTestCase {
            description: "a native route names the adapter and each function it leaves alone",
            route: AnalysisRoute::Native(&PivotlessDialect),
            functions: &["UPPER", "IFF"],
            expected_warning: Some(
                "Adapter 'acme' defines its own nullability rule for UPPER, IFF; native SQL \
                 analysis does not apply it, so these calls take the dialect's own nullability",
            ),
        },
        UnappliedRulesTestCase {
            description: "Polyglot calls the adapter's rules, so nothing is unapplied",
            route: AnalysisRoute::Legacy,
            functions: &["UPPER"],
            expected_warning: None,
        },
        UnappliedRulesTestCase {
            description: "no model is analysed where no parser reads the dialect",
            route: AnalysisRoute::Unavailable,
            functions: &["UPPER"],
            expected_warning: None,
        },
        UnappliedRulesTestCase {
            description: "an adapter with no rules of its own has nothing to warn about",
            route: AnalysisRoute::Native(&PivotlessDialect),
            functions: &[],
            expected_warning: None,
        },
    ];
    for test_case in test_cases {
        let functions: Vec<String> = test_case
            .functions
            .iter()
            .map(|f| (*f).to_owned())
            .collect();
        assert_eq!(
            unapplied_nullability_rules(&test_case.route, "acme", &functions),
            test_case.expected_warning.map(str::to_owned),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_findings_when_analysing_then_they_take_sqlbuild_codes_and_positions() {
    let test_cases = [
        FindingTestCase {
            description: "a VARCHAR column as a condition is an implicit conversion, blocked as B211",
            dialect: &PivotlessDialect,
            sql: "SELECT id FROM orders WHERE status",
            extension_functions: &[],
            expected_findings: &[("B211", Some((1, 29)))],
        },
        FindingTestCase {
            description: "a cast to a type the engine lacks is blocked as B218, at the value cast",
            dialect: &PivotlessDialect,
            sql: "SELECT CAST(id AS NOT_A_TYPE) AS x FROM orders",
            extension_functions: &[],
            expected_findings: &[("B218", Some((1, 13)))],
        },
        FindingTestCase {
            description: "an unknown column on its own line keeps its line and column",
            dialect: &PivotlessDialect,
            sql: "SELECT id,\n  nope\nFROM orders",
            extension_functions: &[],
            expected_findings: &[("B002", Some((2, 3)))],
        },
        FindingTestCase {
            description: "a truth-value test the warehouse rejects is SQLBuild's own B103, after the parse's findings",
            dialect: &FoldingDialect,
            sql: "SELECT id, nope FROM orders WHERE (id > 1) IS NOT TRUE",
            extension_functions: &[],
            expected_findings: &[("B002", Some((1, 12))), ("B103", Some((1, 44)))],
        },
        FindingTestCase {
            description: "a function the catalogue does not know is B101",
            dialect: &CataloguedDialect,
            sql: "SELECT CURRENT_TIMESTAMP() AS created_at FROM orders",
            extension_functions: &[],
            expected_findings: &[("B101", None)],
        },
        FindingTestCase {
            description: "a declared extension function is known, matched without case",
            dialect: &CataloguedDialect,
            sql: "SELECT CURRENT_TIMESTAMP() AS created_at FROM orders",
            extension_functions: &["current_timestamp"],
            expected_findings: &[],
        },
        FindingTestCase {
            description: "an undeclared function is still unknown beside declared extensions",
            dialect: &CataloguedDialect,
            sql: "SELECT CURRENT_TIMESTAMP() AS created_at FROM orders",
            extension_functions: &["md_user_info"],
            expected_findings: &[("B101", None)],
        },
        FindingTestCase {
            description: "Snowflake's arm reports an unknown column at its line and column",
            dialect: &SnowflakeDialect,
            sql: "SELECT id,\n  nope\nFROM orders",
            extension_functions: &[],
            expected_findings: &[("B002", Some((2, 3)))],
        },
        FindingTestCase {
            description: "DuckDB's arm reports an unknown column at its line and column",
            dialect: &DuckDbDialect,
            sql: "SELECT id,\n  nope\nFROM orders",
            extension_functions: &[],
            expected_findings: &[("B002", Some((2, 3)))],
        },
        FindingTestCase {
            description: "SQLite's arm reports an unknown column at its line and column",
            dialect: &SqliteDialect,
            sql: "SELECT id,\n  nope\nFROM orders",
            extension_functions: &[],
            expected_findings: &[("B002", Some((2, 3)))],
        },
        FindingTestCase {
            description: "DuckDB's arm reports a misspelt function as B101",
            dialect: &DuckDbDialect,
            sql: "SELECT lenght(status) AS n FROM orders",
            extension_functions: &[],
            expected_findings: &[("B101", Some((1, 8)))],
        },
    ];
    for test_case in test_cases {
        let analysis: NativeAnalysis = analysed(
            test_case.dialect,
            test_case.sql,
            test_case.extension_functions,
        );
        let findings: Vec<(&str, Option<(usize, usize)>)> = analysis
            .diagnostics
            .iter()
            .map(|row| (row.code.as_str(), row.line.zip(row.column)))
            .collect();
        assert_eq!(
            findings, test_case.expected_findings,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_query_over_referenced_columns_when_analysing_then_lineage_reads_the_resources() {
    let test_cases = [
        LineageTestCase {
            description: "direct, cast and constant columns read the model; an unresolved one is \
                          read from the one relation at medium confidence",
            sql: "SELECT id, CAST(id AS TEXT) AS id_text, 1 AS one, nope FROM orders \
                  WHERE status = 'paid'",
            expected_lineage: &[
                ("id", 0, 1, &["id"]),
                ("id_text", 1, 1, &["id"]),
                ("one", 5, 1, &[]),
                ("nope", 0, 2, &["nope"]),
            ],
            expected_row_sources: &[("orders", "status")],
        },
        LineageTestCase {
            description: "a query without a filter has no columns deciding rows",
            sql: "SELECT status FROM orders",
            expected_lineage: &[("status", 0, 1, &["status"])],
            expected_row_sources: &[],
        },
    ];
    for test_case in test_cases {
        let analysis: NativeAnalysis = analysed(&PivotlessDialect, test_case.sql, &[]);
        let lineage: Vec<(&str, u8, u8, Vec<&str>)> = analysis
            .lineage
            .iter()
            .map(|row| {
                (
                    row.output_column.as_str(),
                    row.transform_code,
                    row.confidence_code,
                    resource_columns(row),
                )
            })
            .collect();
        let row_sources: Vec<(&str, &str)> = analysis
            .row_sources
            .iter()
            .map(|(relation, column)| (relation.as_str(), column.as_str()))
            .collect();
        assert_eq!(
            (lineage, row_sources),
            (
                test_case
                    .expected_lineage
                    .iter()
                    .map(|&(name, transform, confidence, columns)| {
                        (name, transform, confidence, columns.to_vec())
                    })
                    .collect::<Vec<_>>(),
                test_case.expected_row_sources.to_vec()
            ),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_schema_columns_when_analysing_then_they_keep_their_types_and_nullability() {
    let test_cases = [
        ColumnTestCase {
            description: "a non-null INTEGER column",
            sql: "SELECT id FROM orders",
            expected_columns: &[("id", Some("INTEGER"), "non_null")],
        },
        ColumnTestCase {
            description: "a nullable VARCHAR column",
            sql: "SELECT status FROM orders",
            expected_columns: &[("status", Some("VARCHAR"), "nullable")],
        },
    ];
    for test_case in test_cases {
        let analysis: NativeAnalysis = analysed(&PivotlessDialect, test_case.sql, &[]);
        let columns: Vec<(&str, Option<&str>, &str)> = analysis
            .columns
            .iter()
            .map(|column| {
                (
                    column.name.as_str(),
                    column.data_type.as_deref(),
                    column.nullability.as_str(),
                )
            })
            .collect();
        assert_eq!(
            columns, test_case.expected_columns,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_star_when_analysing_then_it_is_resolved_only_over_known_columns() {
    let test_cases = [
        StarTestCase {
            description: "a star over a known relation projects its columns",
            sql: "SELECT * FROM orders",
            expected_star: (true, true, 2),
        },
        StarTestCase {
            description: "a star over an unknown relation leaves columns out",
            sql: "SELECT * FROM elsewhere",
            expected_star: (true, false, 0),
        },
    ];
    for test_case in test_cases {
        let analysis: NativeAnalysis = analysed(&PivotlessDialect, test_case.sql, &[]);
        assert_eq!(
            (
                analysis.has_star,
                analysis.star_resolved,
                analysis.columns.len()
            ),
            test_case.expected_star,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_macro_body_when_reading_it_then_only_one_expression_is_read() {
    let test_cases = [
        MacroBodyTestCase {
            description: "one expression is the body",
            body: "a + 1",
            expected_gap: None,
        },
        MacroBodyTestCase {
            description: "two expressions are not one body",
            body: "a, b",
            expected_gap: Some(("not one expression", 0)),
        },
        MacroBodyTestCase {
            description: "a query is not a body",
            body: "1 FROM t",
            expected_gap: Some(("not one expression", 0)),
        },
        MacroBodyTestCase {
            description: "a refused construct is placed in the body, not the query around it",
            body: "x PIVOT",
            expected_gap: Some(("PIVOT", 2)),
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            PivotlessDialect
                .macro_body(test_case.body)
                .err()
                .map(|gap| (gap.construct, gap.offset)),
            test_case
                .expected_gap
                .map(|(construct, offset)| (construct.to_owned(), offset)),
            "{}",
            test_case.description
        );
    }
}
