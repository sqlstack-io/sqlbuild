use serde_json::{Value, json};
use sqlbuild_sqltext::sql_references::main::extract_sql_references::extract_sql_references;
use sqlbuild_sqltext::sql_references::models::{ReferenceExtraction, SqlReference};
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

use crate::compiler::_helpers::sql_tests::rendering::sql_test_parser;
use crate::compiler::main::sql_scenario_classification::extract_scenario;
use crate::compiler::main::sql_test_extraction::extract_batch_json;
use crate::compiler::models::SqlTestCteText;
use crate::sql_test_attachment::models::{ModelTestCtes, SqlTestHelperProblem};
use crate::sql_test_attachment::tests::test_types::AsciiMatcher;
use crate::sql_test_attachment::types::SqlTestTextMatcher;

/// A scenario's classification as `{"scenario": ...}`, or its error as `{"error": ...}`.
pub(crate) fn scenario_json(sql: &str, file_label: &str, syntax: &LexicalSyntax) -> String {
    extract_scenario(sql, file_label, (syntax, sql_test_parser(Some("duckdb"))))
        .map_or_else(
            |error| json!({"error": error}),
            |scenario| json!({"scenario": scenario}),
        )
        .to_string()
}

/// The batch response of one extraction request, as JSON.
pub(crate) fn batch_outcome(request: &str) -> String {
    extract_batch_json(request).expect("the request is valid")
}

/// One generic-SQL test's payload before (`raw`) or after expansion, and its P012 flag.
pub(crate) fn parity_payload(sql: &str, mode: &str, raw: bool) -> (Value, bool) {
    let request: String = json!({"tests": [{"sql": sql, "fileLabel": "tests/unit/example.sql", "mode": mode, "raw": raw}]})
        .to_string();
    let mut response: Value =
        serde_json::from_str(&batch_outcome(&request)).expect("the response is JSON");
    let invalid_calls: bool = response["tests"][0]
        .as_object_mut()
        .and_then(|test| test.remove("invalidCalls"))
        .and_then(|flag| flag.as_bool())
        .unwrap_or(false);
    (response, invalid_calls)
}

/// A scenario's outcome under the lexical rules given as JSON.
pub(crate) fn scenario_outcome(sql: &str, file_label: &str, syntax: &str) -> String {
    let syntax: LexicalSyntax = serde_json::from_str(syntax).expect("the syntax is JSON");
    scenario_json(sql, file_label, &syntax)
}

/// The unique `__ref` targets the assertion bodies read, under the lexical rules given as JSON.
pub(crate) fn assertion_targets(assertions: &[&str], syntax: &str) -> Vec<String> {
    let syntax: LexicalSyntax = serde_json::from_str(syntax).expect("the syntax is JSON");
    let mut targets: Vec<String> = Vec::new();
    for reference in assertions
        .iter()
        .flat_map(|sql| scanned_references(sql, &syntax))
        .filter(|reference| reference.kind == "ref")
    {
        targets.push(reference.name);
    }
    targets.dedup();
    targets
}

fn scanned_references(sql: &str, syntax: &LexicalSyntax) -> Vec<SqlReference> {
    let ReferenceExtraction::Extracted(scan) = extract_sql_references(sql, syntax) else {
        panic!("the assertion scans");
    };
    scan.references
}

/// Generic SQL lexical rules: `--` line comments.
pub(crate) fn generic_dash_syntax() -> LexicalSyntax {
    LexicalSyntax {
        line_comment_prefixes: vec!["--".to_owned()],
        ..LexicalSyntax::default()
    }
}

fn is_word(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_' || character == '$'
}

fn starts_identifier(token: &&str) -> bool {
    token
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
}

impl SqlTestTextMatcher for AsciiMatcher {
    type Error = ();

    fn identifier_tokens(&self, sql: &str) -> Result<Vec<String>, ()> {
        let mut tokens: Vec<String> = Vec::new();
        for token in sql
            .split(|character: char| !is_word(character))
            .filter(starts_identifier)
        {
            tokens.push(token.to_ascii_lowercase());
        }
        let mut seen: Vec<String> = Vec::new();
        tokens.retain(|token| {
            let fresh: bool = !seen.contains(token);
            seen.push(token.clone());
            fresh
        });
        Ok(tokens)
    }

    fn reads_word(&self, sql: &str, word: &str) -> Result<bool, ()> {
        Ok(sql
            .to_ascii_lowercase()
            .split(|character: char| !is_word(character))
            .any(|token| token == word.to_ascii_lowercase()))
    }
}

pub(crate) fn owned(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}

pub(crate) fn cte_texts(ctes: &[(&str, &str)]) -> Vec<SqlTestCteText> {
    ctes.iter()
        .map(|(name, body)| SqlTestCteText((*name).to_owned(), (*body).to_owned()))
        .collect()
}

/// A model test's CTE view with only model and source mocks.
pub(crate) fn model_test_ctes<'a>(
    ctes: (
        &'a [SqlTestCteText],
        &'a [SqlTestCteText],
        &'a [SqlTestCteText],
    ),
    mocks: (&'a [String], &'a [String]),
) -> ModelTestCtes<'a> {
    ModelTestCtes {
        authored: ctes.0,
        expected: ctes.1,
        assertions: ctes.2,
        mock_models: mocks.0,
        mock_sources: mocks.1,
        mock_seeds: &[],
        mock_dbt_refs: &[],
        mock_table_functions: &[],
    }
}

/// `message @ cte` or `message @ cte: call`.
pub(crate) fn problem_summary(problem: &SqlTestHelperProblem) -> String {
    problem.call.as_ref().map_or_else(
        || format!("{} @ {}", problem.message, problem.cte_name),
        |call| format!("{} @ {}: {call}", problem.message, problem.cte_name),
    )
}
