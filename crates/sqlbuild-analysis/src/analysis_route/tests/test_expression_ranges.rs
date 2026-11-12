use crate::analysis_route::_helpers::analysis::native_sharing::rebase_ranges;
use crate::analysis_route::main::query_expression_ranges::query_expression_ranges;
use crate::analysis_route::models::{DuckDbDialect, NativeAnalysis};
use crate::analysis_route::tests::helpers::analysed;
use crate::analysis_route::types::NativeDialect;
use crate::semantic_validation::_helpers::normalization::normalize_native_sql_mapped;
use crate::sql_macros::main::typed_analysis_sql::typed_analysis_sql;
use crate::sql_macros::models::{TextSplice, TypedCall};
use sqlbuild_attachments::cursor_intrinsics::main::replace_intrinsics_with_spans::replace_intrinsics_with_spans;

/// Each output column with the text of `query_sql` its mapped expression range covers, after
/// `passes` rewrote `query_sql` into `analysis_sql`.
fn mapped_texts<'a>(
    (query_sql, analysis_sql): (&'a str, &str),
    placeholders: &[(String, String)],
    passes: &[Vec<TextSplice>],
) -> Vec<(String, Option<&'a str>)> {
    let dialect: &DuckDbDialect = &DuckDbDialect;
    let (cleaned, _) =
        normalize_native_sql_mapped(analysis_sql, "duckdb", dialect.stub_prefix(), placeholders)
            .expect("the test query normalises");
    let analysis: NativeAnalysis = analysed(dialect, &cleaned, &[]);
    let ranges: Vec<Option<(usize, usize)>> = analysis
        .lineage
        .iter()
        .map(|row| row.expression_range)
        .collect();
    let mapped = query_expression_ranges("duckdb", (analysis_sql, placeholders), passes, &ranges);
    analysis
        .lineage
        .iter()
        .zip(mapped)
        .map(|(row, range)| {
            (
                row.output_column.clone(),
                range.map(|(start, end)| &query_sql[start..end]),
            )
        })
        .collect()
}

#[test]
fn given_compiled_sql_that_cleaning_rewrites_when_mapping_ranges_then_they_cover_its_own_text() {
    let placeholders: Vec<(String, String)> = vec![("floor".to_owned(), "100000".to_owned())];
    let query_sql: &str = "-- café ünïcode\nWITH x AS (SELECT * FROM __ref(\"orders\") WHERE id > @@@floor \
                           AND id < __cursor_end())\nSELECT x.id AS id, UPPER(x.status) AS loud, 'é' AS mark FROM x";
    assert_eq!(
        mapped_texts((query_sql, query_sql), &placeholders, &[]),
        vec![
            ("id".to_owned(), Some("x.id AS id")),
            ("loud".to_owned(), Some("UPPER(x.status) AS loud")),
            ("mark".to_owned(), Some("'é' AS mark")),
        ]
    );
}

#[test]
fn given_typed_calls_and_cursor_bounds_when_mapping_ranges_then_they_cover_the_compiled_text() {
    let query_sql: &str = "SELECT ROUND(amount * 1.2, 2) AS gross, id AS id FROM orders \
                           WHERE updated_at < __cursor_end()";
    let call_start: usize = query_sql.find("ROUND").expect("the call is in the query");
    let call_end: usize = query_sql
        .find(" AS gross")
        .expect("the alias is in the query");
    let calls: Vec<TypedCall> = vec![TypedCall {
        start: call_start,
        end: call_end,
        macro_name: "gross_amount".to_owned(),
        returns: Some("DECIMAL(18, 2)".to_owned()),
        returns_like: None,
    }];
    let (typed, typed_splices) = typed_analysis_sql(query_sql, &calls, "duckdb");
    assert_ne!(typed, query_sql, "the call is marked for analysis");
    let (analysis_sql, spans) = replace_intrinsics_with_spans(&typed, "Model SQL", |_| {
        "CAST('2000-01-01' AS TIMESTAMP)".to_owned()
    })
    .expect("the cursor call is replaced");
    let cursor_splices: Vec<TextSplice> = spans
        .iter()
        .map(|span| TextSplice {
            source: (span.source_start, span.source_end),
            output: (span.output_start, span.output_end),
        })
        .collect();
    assert_eq!(
        mapped_texts(
            (query_sql, &analysis_sql),
            &[],
            &[typed_splices, cursor_splices]
        ),
        vec![
            ("gross".to_owned(), Some("ROUND(amount * 1.2, 2) AS gross")),
            ("id".to_owned(), Some("id AS id")),
        ]
    );
}

#[test]
fn given_a_shared_analysis_when_rebasing_onto_a_member_then_ranges_follow_its_relation_names() {
    let leader: &str =
        "WITH x AS (SELECT * FROM orders) SELECT x.id AS id, UPPER(x.status) AS loud FROM x";
    let member: &str = "WITH x AS (SELECT * FROM customer_orders) SELECT x.id AS id, UPPER(x.status) AS loud FROM x";
    let mut analysis: NativeAnalysis = analysed(&DuckDbDialect, leader, &[]);
    rebase_ranges(&mut analysis, leader, member);
    let texts: Vec<Option<&str>> = analysis
        .lineage
        .iter()
        .map(|row| row.expression_range.map(|(start, end)| &member[start..end]))
        .collect();
    assert_eq!(
        texts,
        vec![Some("x.id AS id"), Some("UPPER(x.status) AS loud")]
    );
}
