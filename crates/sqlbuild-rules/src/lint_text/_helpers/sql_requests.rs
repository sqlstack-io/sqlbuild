//! Native SQL lint request payloads and the batch engine call.

use std::borrow::Cow;

use serde_json::{Map, Value};
use sqlbuild_attachments::ceremonial_select::main::omitted_select_offset::omitted_select_offset;
use sqlbuild_attachments::ceremonial_select::models::OmittedSelect;
use sqlbuild_core::constants::PROJECT_CONFIG_FILENAME;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

use crate::lint_text::_helpers::sql_diagnostics::{
    sql_macro_output_spans, without_generated_comments,
};
use crate::lint_text::constants::{
    CROSS_KEYWORD, CTE_ARGUMENT, EMPTY_FIXTURE_RELATION, FIXTURE_CTE_MARKERS,
    FIXTURE_HARNESS_CTE_PREFIXES, INVALID_BATCH, LINE_COMMENT_PREFIX, LINT_API_VERSION,
    LONG_LITERAL_GUIDANCE, OMITTED_CEREMONIAL_SELECT_SQL,
};
use crate::lint_text::models::{LintSqlBody, LintSqlOptions};
use sqlclean::sql_lint::main::batch_engine::lint_batch_json;

/// Where a fixture body omits its ceremonial `SELECT 1`, so the parser sees one statement.
pub(crate) fn omitted_select(body: &LintSqlBody) -> Option<usize> {
    if !body.allows_ceremonial_select {
        return None;
    }
    let syntax: LexicalSyntax = LexicalSyntax {
        line_comment_prefixes: vec![LINE_COMMENT_PREFIX.to_string()],
        ..LexicalSyntax::default()
    };
    match omitted_select_offset(&body.lint_text, &syntax) {
        OmittedSelect::At(offset) => Some(offset),
        OmittedSelect::Absent => None,
    }
}

/// Whether `text` may hold a `CROSS JOIN`; other bodies omit relation names to share requests.
fn mentions_cross_join(text: &str) -> bool {
    text.as_bytes()
        .windows(CROSS_KEYWORD.len())
        .any(|window| window.eq_ignore_ascii_case(CROSS_KEYWORD))
}

/// The names a `@sql.` call passes as whole arguments, such as `orders` in `@sql.net_amount(orders)`: the relations a plain slot read in its output may qualify with. Slot mappings and the reserved `__cte` name pass none.
fn call_relations(call: &str) -> Vec<String> {
    let Some(open) = call.find('(') else {
        return Vec::new();
    };
    let close: usize = call
        .rfind(')')
        .filter(|&close| close > open)
        .unwrap_or(call.len());
    let mut names: Vec<String> = Vec::new();
    for argument in top_level_arguments(&call[open + 1..close]) {
        if let Some(name) = passed_name(argument) {
            names.push(name.to_owned());
        }
    }
    names
}

/// The bare name one call argument passes, if it passes one: the value of `name := value` or the whole argument.
fn passed_name(argument: &str) -> Option<&str> {
    let value: &str = match argument.split_once(":=") {
        Some((name, _)) if name.contains('.') || name.trim().eq_ignore_ascii_case(CTE_ARGUMENT) => {
            return None;
        }
        Some((_, value)) => value,
        None => argument,
    };
    let name: &str = value.trim().trim_matches('"');
    let bare: bool = !name.is_empty()
        && name
            .chars()
            .all(|character| character.is_alphanumeric() || character == '_');
    bare.then_some(name)
}

/// `arguments` split at its top-level commas, outside brackets and quotes.
fn top_level_arguments(arguments: &str) -> Vec<&str> {
    let mut parts: Vec<&str> = Vec::new();
    let (mut depth, mut quote, mut start): (usize, Option<char>, usize) = (0, None, 0);
    for (index, character) in arguments.char_indices() {
        match (quote, character) {
            (Some(open), _) if character == open => quote = None,
            (Some(_), _) => {}
            (None, '\'' | '"') => quote = Some(character),
            (None, '(' | '[') => depth += 1,
            (None, ')' | ']') => depth = depth.saturating_sub(1),
            (None, ',') if depth == 0 => {
                parts.push(&arguments[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    parts.push(&arguments[start..]);
    parts
}

/// The harness SQLBuild wraps a test or macro body in; its inputs may read the empty fixture.
fn fixture_harness(body: &LintSqlBody) -> Value {
    let mut harness: Map<String, Value> = Map::new();
    let _ = harness.insert(
        "cte_prefixes".to_string(),
        Value::from(FIXTURE_HARNESS_CTE_PREFIXES.to_vec()),
    );
    if body.allows_empty_fixture_star {
        let _ = harness.insert(
            "input_cte_prefixes".to_string(),
            Value::from(FIXTURE_CTE_MARKERS.to_vec()),
        );
        let _ = harness.insert(
            "empty_relation".to_string(),
            Value::from(EMPTY_FIXTURE_RELATION),
        );
    }
    Value::Object(harness)
}

fn strings(values: &[String]) -> Value {
    Value::from(values.to_vec())
}

/// The engine request for one body, with the omitted `SELECT 1` added back.
pub(crate) fn lint_payload(
    (body, file): (&LintSqlBody, &str),
    omitted: Option<usize>,
    options: &LintSqlOptions,
) -> Value {
    let lint_text: Cow<'_, str> = without_generated_comments(body, (file, &options.dialect));
    let inserted: usize = OMITTED_CEREMONIAL_SELECT_SQL.chars().count();
    let outputs: Vec<Value> = sql_macro_output_spans(body, (file, &options.dialect))
        .into_iter()
        .map(|(start, end, call)| {
            let (start, end): (usize, usize) = match omitted {
                Some(offset) if start >= offset => (start + inserted, end + inserted),
                _ => (start, end),
            };
            serde_json::json!({"start": start, "end": end, "relations": call_relations(&call)})
        })
        .collect();
    let sql: String = match omitted {
        Some(offset) => {
            let characters: Vec<char> = lint_text.chars().collect();
            let head: String = characters[..offset].iter().collect();
            let tail: String = characters[offset..].iter().collect();
            format!("{head}{OMITTED_CEREMONIAL_SELECT_SQL}{tail}")
        }
        None => lint_text.into_owned(),
    };
    let mut payload: Map<String, Value> = Map::new();
    let _ = payload.insert("version".to_string(), Value::from(LINT_API_VERSION));
    let _ = payload.insert("sql".to_string(), Value::from(sql));
    let _ = payload.insert("dialect".to_string(), Value::from(options.dialect.clone()));
    let _ = payload.insert(
        "config_file".to_string(),
        Value::from(PROJECT_CONFIG_FILENAME),
    );
    let _ = payload.insert(
        "long_literal_guidance".to_string(),
        Value::from(LONG_LITERAL_GUIDANCE),
    );
    if !outputs.is_empty() {
        let _ = payload.insert("sql_macro_outputs".to_string(), Value::from(outputs));
    }
    if let Some(enabled) = &options.enabled_rules {
        let _ = payload.insert("enabled_rules".to_string(), strings(enabled));
    }
    let lists: [(&str, &[String]); 4] = [
        ("ignored_rules", &options.ignored_rules),
        ("external_identifiers", &body.external_identifiers),
        ("dependency_identifiers", &body.dependency_identifiers),
        (
            "externally_referenced_ctes",
            &body.externally_referenced_ctes,
        ),
    ];
    for (key, values) in lists {
        if !values.is_empty() {
            let _ = payload.insert(key.to_string(), strings(values));
        }
    }
    if body.allows_dynamic_output_star {
        let _ = payload.insert("allows_dynamic_output_star".to_string(), Value::Bool(true));
    }
    if body.allows_ceremonial_select {
        let _ = payload.insert("fixture_harness".to_string(), fixture_harness(body));
    }
    let _ = payload.insert(
        "max_literal_length".to_string(),
        Value::from(options.max_literal_length),
    );
    let _ = payload.insert(
        "max_ranking_order_by".to_string(),
        Value::from(options.max_ranking_order_by),
    );
    if !body.relation_keys.is_empty() {
        let keys: Map<String, Value> = body
            .relation_keys
            .iter()
            .map(|(sentinel, keys)| (sentinel.clone(), Value::from(keys.clone())))
            .collect();
        let _ = payload.insert("relation_keys".to_string(), Value::Object(keys));
    }
    if !body.relation_names.is_empty() && mentions_cross_join(&body.lint_text) {
        let names: Map<String, Value> = body
            .relation_names
            .iter()
            .map(|(sentinel, name)| (sentinel.clone(), Value::from(name.clone())))
            .collect();
        let _ = payload.insert("relation_names".to_string(), Value::Object(names));
    }
    Value::Object(payload)
}

/// Run every payload through the batch engine, one response per payload.
pub(crate) fn run_batch(payloads: &[Value]) -> Result<Vec<Value>, String> {
    if payloads.is_empty() {
        return Ok(Vec::new());
    }
    let response: String = lint_batch_json(&Value::from(payloads.to_vec()).to_string())?;
    match serde_json::from_str::<Value>(&response).map_err(|error| error.to_string())? {
        Value::Array(responses) if responses.len() == payloads.len() => Ok(responses),
        _ => Err(INVALID_BATCH.to_string()),
    }
}
