use crate::analysis_route::main::skipped_analysis::{skipped_analysis, unavailable_analysis};
use crate::analysis_route::models::{
    AnalysisInput, LocatedGap, NativeAnalysis, StandInSchema, TypeMatch,
};
use crate::analysis_route::tests::test_types::{
    CataloguedDialect, FindingTestCase, MacroBodyTestCase, NativeTypeMatchTestCase,
    PivotlessDialect, ReadWholeTestCase,
};
use crate::analysis_route::types::NativeDialect;
use crate::assembly::analysis_session::models::LineageRow;

/// `orders` with a non-null INTEGER id and a nullable VARCHAR status.
fn orders() -> StandInSchema {
    StandInSchema::new(
        "orders",
        &[("id", "INTEGER", false), ("status", "VARCHAR", true)],
    )
}

/// The analysis of `sql` over `orders`, which the model references as the model `shop.orders`.
fn analysed(
    dialect: &dyn NativeDialect,
    sql: &str,
    extension_functions: &[&str],
) -> NativeAnalysis {
    let schema: StandInSchema = orders();
    let references: [(String, String, String); 1] = [(
        "orders".to_owned(),
        "model".to_owned(),
        "shop.orders".to_owned(),
    )];
    let extension_functions: Vec<String> = extension_functions
        .iter()
        .map(|name| (*name).to_owned())
        .collect();
    dialect
        .analyse(&AnalysisInput {
            sql,
            schema: &schema,
            references: &references,
            extension_functions: &extension_functions,
        })
        .expect("the test query is read whole")
}

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
fn given_a_parser_message_or_no_parser_when_skipping_analysis_then_p026_says_why() {
    let syntax: LocatedGap = LocatedGap {
        construct: "syntax error".to_owned(),
        message: "syntax error at or near \"FROM\"".to_owned(),
        line: 1,
        column: 15,
        syntax: true,
    };
    assert_eq!(
        skipped_analysis("orders", "postgres", &syntax),
        (
            "P026",
            "SQL analysis skipped for model 'orders': the postgres parser cannot read syntax \
             error at line 1, column 15: syntax error at or near \"FROM\""
                .to_owned()
        ),
        "the parser's message follows the construct"
    );
    assert_eq!(
        unavailable_analysis("orders", Some("trino")),
        (
            "P026",
            "SQL analysis skipped for model 'orders': no native parser reads the trino dialect"
                .to_owned()
        ),
        "an unknown dialect is named"
    );
    assert_eq!(
        unavailable_analysis("orders", None).1,
        "SQL analysis skipped for model 'orders': no native parser reads the generic dialect",
        "no dialect is the generic dialect"
    );
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
    ];
    for test_case in test_cases {
        let findings: Vec<(String, Option<usize>, Option<usize>)> = analysed(
            test_case.dialect,
            test_case.sql,
            test_case.extension_functions,
        )
        .diagnostics
        .into_iter()
        .map(|row| (row.code, row.line, row.column))
        .collect();
        let expected: Vec<(String, Option<usize>, Option<usize>)> = test_case
            .expected_findings
            .iter()
            .map(|&(code, position)| {
                (
                    code.to_owned(),
                    position.map(|(line, _)| line),
                    position.map(|(_, column)| column),
                )
            })
            .collect();
        assert_eq!(findings, expected, "{}", test_case.description);
    }
}

#[test]
fn given_a_query_over_referenced_columns_when_analysing_then_lineage_reads_the_resources() {
    let analysis: NativeAnalysis = analysed(
        &PivotlessDialect,
        "SELECT id, CAST(id AS TEXT) AS id_text, 1 AS one, nope FROM orders WHERE status = 'paid'",
        &[],
    );
    let source = |column: &str| {
        (
            "model".to_owned(),
            "shop.orders".to_owned(),
            column.to_owned(),
        )
    };
    assert_eq!(
        analysis.lineage,
        vec![
            LineageRow {
                output_column: "id".to_owned(),
                transform_code: 0,
                confidence_code: 1,
                sources: vec![source("id")],
            },
            LineageRow {
                output_column: "id_text".to_owned(),
                transform_code: 1,
                confidence_code: 1,
                sources: vec![source("id")],
            },
            LineageRow {
                output_column: "one".to_owned(),
                transform_code: 5,
                confidence_code: 1,
                sources: Vec::new(),
            },
            LineageRow {
                output_column: "nope".to_owned(),
                transform_code: 0,
                confidence_code: 2,
                sources: vec![source("nope")],
            },
        ],
        "direct, cast and constant columns read the referenced model; an unresolved column is \
         read from the one relation at medium confidence"
    );
    assert_eq!(
        analysis.row_sources,
        vec![("orders".to_owned(), "status".to_owned())],
        "the filter's column decides rows"
    );
    assert_eq!(
        (
            analysis.columns[0].data_type.clone(),
            analysis.columns[0].nullability.as_str()
        ),
        (
            PivotlessDialect
                .normalize_type("INTEGER")
                .map(|normalized| normalized.normalized_name),
            "non_null"
        ),
        "a column is typed through the dialect and keeps the schema's nullability"
    );
}

#[test]
fn given_a_star_when_analysing_then_it_is_resolved_only_over_known_columns() {
    let known: NativeAnalysis = analysed(&PivotlessDialect, "SELECT * FROM orders", &[]);
    assert_eq!(
        (known.has_star, known.star_resolved, known.columns.len()),
        (true, true, 2),
        "a star over a known relation projects its columns"
    );
    let unknown: NativeAnalysis = analysed(&PivotlessDialect, "SELECT * FROM elsewhere", &[]);
    assert_eq!(
        (unknown.has_star, unknown.star_resolved),
        (true, false),
        "a star over an unknown relation leaves columns out"
    );
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
