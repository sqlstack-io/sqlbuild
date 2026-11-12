use crate::analysis_route::_helpers::analysis::native_sharing::rebase_ranges;
use crate::analysis_route::main::query_expression_ranges::query_expression_ranges;
use crate::analysis_route::models::{DuckDbDialect, NativeAnalysis};
use crate::analysis_route::tests::helpers::analysed;
use crate::analysis_route::types::NativeDialect;
use crate::semantic_validation::_helpers::normalization::normalize_native_sql_mapped;

/// Each output column with the text of `query_sql` its mapped expression range covers.
fn mapped_texts<'a>(
    query_sql: &'a str,
    placeholders: &[(String, String)],
) -> Vec<(String, Option<&'a str>)> {
    let dialect: &DuckDbDialect = &DuckDbDialect;
    let (cleaned, _) =
        normalize_native_sql_mapped(query_sql, "duckdb", dialect.stub_prefix(), placeholders)
            .expect("the test query normalises");
    let analysis: NativeAnalysis = analysed(dialect, &cleaned, &[]);
    let ranges: Vec<Option<(usize, usize)>> = analysis
        .lineage
        .iter()
        .map(|row| row.expression_range)
        .collect();
    let mapped = query_expression_ranges("duckdb", (query_sql, placeholders), &ranges);
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
        mapped_texts(query_sql, &placeholders),
        vec![
            ("id".to_owned(), Some("x.id AS id")),
            ("loud".to_owned(), Some("UPPER(x.status) AS loud")),
            ("mark".to_owned(), Some("'é' AS mark")),
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
