use sqlbuild_host::compile_context::main::collect_compile_timings::collect_compile_timings;
use sqlbuild_host::compile_context::main::timing_milliseconds::timing_milliseconds;

use crate::assembly::analysis_session::models::{LineageFacts, ModelOutcome, SessionRequest};
use crate::assembly::analysis_session::tests::helpers::{completed, model, shapes, started};
use crate::assembly::analysis_session::types::Shapes;

/// `request`'s analysis under a compile metric collector: each model's lines, and the sharing counts.
fn analysed(request: SessionRequest) -> (Vec<Vec<String>>, Vec<(&'static str, u64)>) {
    collect_compile_timings(|collector| {
        let (_, outcomes, _) = completed(started(request));
        let metrics: Vec<(&str, u64)> = timing_milliseconds(collector)
            .into_iter()
            .filter(|(name, _)| name.starts_with("native_analysis_"))
            .collect();
        Ok::<_, ()>((outcomes.iter().map(outcome_lines).collect(), metrics))
    })
    .expect("the session completes")
}

/// `SELECT id, amount + 1 AS bumped` from `source`.
fn bumped(source: &str) -> String {
    format!("SELECT id, amount + 1 AS bumped FROM __source(\"{source}\")")
}

/// Each model's output columns, then each output's lineage sources.
fn outcome_lines(outcome: &ModelOutcome) -> Vec<String> {
    let analysis = &outcome.analysis;
    let mut lines: Vec<String> = analysis
        .columns
        .iter()
        .flatten()
        .map(|column| {
            let data_type: &str = column.data_type.as_deref().unwrap_or("-");
            format!("{} {data_type}", column.name)
        })
        .collect();
    let (LineageFacts::Native(rows) | LineageFacts::NativeFacts(rows)) = &analysis.lineage;
    for row in rows {
        let sources: Vec<String> = row
            .sources
            .iter()
            .map(|(kind, name, column)| format!("{kind}:{name}.{column}"))
            .collect();
        lines.push(format!("{} <- {}", row.output_column, sources.join(" ")));
    }
    lines
}

/// A DuckDB request over the sources `relations`, with each model's name, SQL and sources.
fn sources_request(relations: &Shapes, models: &[(&str, String, &[&str])]) -> SessionRequest {
    let nullability: Shapes = relations
        .iter()
        .map(|(name, columns)| {
            let unknown = columns
                .iter()
                .map(|(column, _)| (column.clone(), "unknown".to_owned()))
                .collect();
            (name.clone(), unknown)
        })
        .collect();
    SessionRequest {
        dialect: "duckdb".to_owned(),
        case_sensitive_shapes: false,
        quoted_ignore_case: false,
        function_return_types: Vec::new(),
        known_functions: Vec::new(),
        nullability_rules: Some(Vec::new()),
        nullability_callback: None,
        rich_type_inference: true,
        forced_order: None,
        retain_relations: false,
        column_types: relations.clone(),
        column_nullability: nullability,
        complete_schemas: relations.clone(),
        catalog_schemas: relations.clone(),
        dynamic_families_by_table: Vec::new(),
        models: models
            .iter()
            .map(|(name, sql, sources)| model(name, sql, sources, &[]))
            .collect(),
    }
}

#[test]
fn given_models_differing_only_in_relation_names_when_analysing_then_share_only_equal_shapes() {
    let relations: Shapes = shapes(&[
        ("src_a", &[("id", "INTEGER"), ("amount", "DOUBLE")]),
        ("src_b", &[("id", "INTEGER"), ("amount", "DOUBLE")]),
        ("src_c", &[("id", "VARCHAR"), ("amount", "DOUBLE")]),
    ]);
    let request: SessionRequest = sources_request(
        &relations,
        &[
            ("from_a", bumped("src_a"), &["src_a"]),
            ("from_b", bumped("src_b"), &["src_b"]),
            ("from_c", bumped("src_c"), &["src_c"]),
        ],
    );

    let (lines, metrics) = analysed(request);

    let expected = |source: &str, id_type: &str| {
        vec![
            format!("id {id_type}"),
            "bumped DOUBLE".to_owned(),
            format!("id <- source:{source}.id"),
            format!("bumped <- source:{source}.amount"),
        ]
    };
    assert_eq!(
        lines,
        [
            expected("src_a", "INTEGER"),
            expected("src_b", "INTEGER"),
            expected("src_c", "VARCHAR"),
        ],
        "each model keeps its own types and lineage, whether it shared an analysis or not"
    );
    assert_eq!(
        metrics,
        [
            ("native_analysis_shared", 1),
            ("native_analysis_unshared", 2)
        ],
        "only the model whose relation has the same shape shares"
    );
}

#[test]
fn given_references_whose_names_sort_differently_when_analysing_then_each_keeps_its_source_order() {
    let relations: Shapes = shapes(&[
        ("src_a", &[("id", "INTEGER")]),
        ("src_b", &[("id", "INTEGER")]),
        ("src_c", &[("id", "INTEGER")]),
        ("src_d", &[("id", "INTEGER")]),
        ("src_e", &[("id", "INTEGER")]),
        ("src_f", &[("id", "INTEGER")]),
    ]);
    let joined = |first: &str, second: &str| {
        format!(
            "SELECT l.id + r.id AS total FROM __source(\"{first}\") AS l \
             JOIN __source(\"{second}\") AS r ON l.id = r.id"
        )
    };
    let request: SessionRequest = sources_request(
        &relations,
        &[
            ("ab", joined("src_a", "src_b"), &["src_a", "src_b"]),
            ("cd", joined("src_c", "src_d"), &["src_c", "src_d"]),
            ("fe", joined("src_f", "src_e"), &["src_f", "src_e"]),
        ],
    );

    let (lines, metrics) = analysed(request);

    let expected = |first: &str, second: &str| {
        vec![
            "total INTEGER".to_owned(),
            format!("total <- source:{first}.id source:{second}.id"),
        ]
    };
    assert_eq!(
        lines,
        [
            expected("src_a", "src_b"),
            expected("src_c", "src_d"),
            expected("src_e", "src_f"),
        ],
        "lineage lists sources by relation name, whichever model's analysis is shared"
    );
    assert_eq!(
        metrics,
        [
            ("native_analysis_shared", 1),
            ("native_analysis_unshared", 2)
        ],
        "only the model whose names sort like the first one's shares"
    );
}
