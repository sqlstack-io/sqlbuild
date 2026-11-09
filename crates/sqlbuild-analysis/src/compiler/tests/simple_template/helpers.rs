use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;
use sqlquack_syntax::{Dialect, dialect_tokens};

use crate::compiler::_helpers::sql_tests::planning::{
    AnalysisTemplate, SqlTestPatterns, parsed_analysis_template,
};
use crate::compiler::_helpers::sql_tests::rendering::sql_test_parser;
use crate::compiler::_helpers::sql_tests::simple_template::simple_analysis_template;
use crate::compiler::tests::test_types::SimpleTemplateCorpusOutcome;

pub(crate) fn template_patterns() -> SqlTestPatterns {
    SqlTestPatterns::new(LexicalSyntax {
        line_comment_prefixes: vec!["--".to_owned()],
        ..LexicalSyntax::default()
    })
    .expect("the SQL-test patterns compile")
}

/// Generated templates, each with every token deleted and with stray tokens inserted before it,
/// counted where the recognizer reads them and listed where its facts differ from the parser's.
pub(crate) fn simple_template_corpus_outcome(dialect_name: &str) -> SimpleTemplateCorpusOutcome {
    let dialect = match dialect_name {
        "duckdb" => Dialect::DuckDB,
        _ => Dialect::Snowflake,
    };
    let parser = sql_test_parser(Some(dialect_name)).expect("a native test dialect");
    let patterns = template_patterns();
    let mut queries: Vec<String> = Vec::new();
    for projection in [
        "id",
        "*",
        "CAST(id AS INTEGER) AS id",
        "COALESCE(id, 0) + id * 2 AS id",
        "CAST((id + 1) / 2 AS DOUBLE) AS id",
        "1.5 AS id",
        "\"__ref(ignored)\" AS id",
        "\"a\"\"b\" AS id",
        "final.id AS \"Final\"",
    ] {
        for relation in [
            "__ref(\"orders\")",
            "__source(\"orders\")",
            "__seed(\"orders\")",
            "__dbt_ref(\"shop\", \"orders\")",
            "\"__ref\"(\"orders\")",
            "__REF(orders)",
            "__ref(\"now\")",
            "__ref(final)",
            "__ref(\"a.b\")",
            "__ref(\"a\"\"b\")",
            "\"__sqb_0\"",
            "final",
        ] {
            let body = format!("SELECT {projection} FROM {relation}");
            for sql in [
                body.clone(),
                format!("{body};"),
                format!("WITH recent AS ({body}) SELECT id FROM recent"),
                format!("{body} UNION ALL BY NAME {body}"),
                format!(
                    "WITH recent AS ({body}) SELECT id FROM __ref(\"customers\") AS c LEFT JOIN recent AS r ON c.id = r.id WHERE r.id = 0"
                ),
            ] {
                let tokens = dialect_tokens(&sql, dialect).expect("generated SQL tokenizes");
                for token in &tokens {
                    queries.push(format!(
                        "{} {}",
                        &sql[..token.span.start],
                        &sql[token.span.end..]
                    ));
                    for inserted in [".", ")", "(", "FROM", "'__ref(\"ignored\")'", "--"] {
                        queries.push(format!(
                            "{} {inserted} {}",
                            &sql[..token.span.start],
                            &sql[token.span.start..]
                        ));
                    }
                }
                queries.push(sql);
            }
        }
    }
    let recognized: Vec<(String, AnalysisTemplate)> = queries
        .into_iter()
        .filter_map(|sql| {
            simple_analysis_template(&sql, dialect_name, &patterns).map(|facts| (sql, facts))
        })
        .collect();
    SimpleTemplateCorpusOutcome {
        recognized: recognized.len(),
        mismatches: recognized
            .into_iter()
            .filter_map(|(sql, fast)| {
                let parsed = parsed_analysis_template(&sql, (parser, &patterns));
                (Some(&fast) != parsed.as_ref())
                    .then(|| format!("{sql}: fast={fast:?}, parsed={parsed:?}"))
            })
            .collect(),
    }
}
