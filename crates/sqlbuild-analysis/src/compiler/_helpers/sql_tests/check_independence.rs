//! Expected results and assertions must not define or read each other, even through helpers.

use std::collections::{BTreeSet, HashMap};

use sqlbuild_core::graph_walk::main::breadth_first_path::breadth_first_path;

use serde_json::Value;
use sqlbuild_sqltext::sql_scan::main::dialect_non_code_end::dialect_non_code_end;
use sqlbuild_sqltext::sql_scan::models::{LexicalSyntax, Unclosed};
use sqlquack_syntax::{Expression, ExpressionWalk, Parsed};

use crate::analysis_route::types::NativeDialect;
use crate::compiler::constants::{ALIAS_FIELD, CTES_FIELD, NAME_FIELD};

const DIRECT_DEPENDENCY_PATH_LENGTH: usize = 2;
const IDENTIFIER_QUOTES: &[u8] = b"\"`";

/// The check CTEs of one file and the labels its messages name.
pub(crate) struct IndependenceCheck<'a> {
    pub(crate) ctes: &'a [(String, String)],
    pub(crate) expected_prefix: &'a str,
    pub(crate) assertion_prefix: &'a str,
    pub(crate) file_label: &'a str,
    pub(crate) context_label: &'a str,
    /// The project's native parser; without one, references come from the identifier scan.
    pub(crate) parser: Option<&'a dyn NativeDialect>,
}

/// The first independence error of `check`, if any.
pub(crate) fn independence_error(
    check: &IndependenceCheck<'_>,
    syntax: &LexicalSyntax,
) -> Option<String> {
    let mut names_by_key: HashMap<String, &str> = HashMap::new();
    for (name, _) in check.ctes {
        names_by_key.insert(casefold(name), name);
    }
    let keys_with_prefix = |prefix: &str| -> BTreeSet<String> {
        names_by_key
            .iter()
            .filter(|(_, name)| name.starts_with(prefix))
            .map(|(key, _)| key.clone())
            .collect()
    };
    let expected: BTreeSet<String> = keys_with_prefix(check.expected_prefix);
    let assertions: BTreeSet<String> = keys_with_prefix(check.assertion_prefix);
    for (name, body) in check.ctes {
        let key: String = casefold(name);
        let (prohibited_prefix, prohibited_label) = if expected.contains(&key) {
            (check.assertion_prefix, "assertion")
        } else if assertions.contains(&key) {
            (check.expected_prefix, "expected result")
        } else {
            continue;
        };
        if !casefold(body).contains(&casefold(prohibited_prefix)) {
            continue;
        }
        if let Some(nested) = defined_cte_names(check.parser, body)
            .into_iter()
            .find(|nested| nested.starts_with(prohibited_prefix))
        {
            return Some(format!(
                "{} '{}' check CTE '{name}' must not define {prohibited_label} CTE '{nested}'; \
                 expected results and assertions must be independent",
                check.context_label, check.file_label
            ));
        }
    }
    if expected.is_empty() || assertions.is_empty() {
        return None;
    }
    let mut dependencies: HashMap<String, Vec<String>> = HashMap::new();
    for (name, body) in check.ctes {
        let references = match known_cte_references(check.parser, body, &names_by_key, syntax) {
            Ok(references) => references,
            Err(message) => return Some(message),
        };
        dependencies.insert(casefold(name), references);
    }
    for (origins, prohibited) in [(&expected, &assertions), (&assertions, &expected)] {
        for origin in origins {
            let Some(path) = dependency_path(origin, prohibited, &dependencies) else {
                continue;
            };
            let rendered: Vec<&str> = path.iter().map(|key| names_by_key[key]).collect();
            let through: String = if rendered.len() > DIRECT_DEPENDENCY_PATH_LENGTH {
                format!(
                    " through {}",
                    rendered[1..rendered.len() - 1]
                        .iter()
                        .map(|name| format!("'{name}'"))
                        .collect::<Vec<String>>()
                        .join(" -> ")
                )
            } else {
                String::new()
            };
            return Some(format!(
                "{} '{}' check CTE '{}' must not depend on '{}'{through}; expected results and \
                 assertions must be independent",
                check.context_label,
                check.file_label,
                rendered[0],
                rendered[rendered.len() - 1]
            ));
        }
    }
    None
}

/// `str.casefold`, through Rust's Unicode lower-case mapping.
fn casefold(text: &str) -> String {
    text.to_lowercase()
}

/// The one statement the project's parser reads from `sql` whole, or `None`.
fn parse_one(parser: Option<&dyn NativeDialect>, sql: &str) -> Option<Expression> {
    match parser?.parse(sql) {
        Ok(Parsed {
            mut statements,
            gaps,
        }) if gaps.is_empty() && statements.len() == 1 => statements.pop(),
        _ => None,
    }
}

/// Every CTE alias defined anywhere in `sql`, in the order `to_dict()` lists them.
fn defined_cte_names(parser: Option<&dyn NativeDialect>, sql: &str) -> Vec<String> {
    let Some(parsed) = parse_one(parser, sql) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::to_value(&parsed) else {
        return Vec::new();
    };
    collect_cte_names(&value, Vec::new())
}

/// `names` followed by each new CTE alias under `value`, depth first.
fn collect_cte_names(value: &Value, mut names: Vec<String>) -> Vec<String> {
    match value {
        Value::Object(fields) => {
            if let Some(Value::Array(ctes)) = fields.get(CTES_FIELD) {
                for cte in ctes {
                    if let Some(Value::String(name)) = cte
                        .get(ALIAS_FIELD)
                        .filter(|alias| alias.is_object())
                        .and_then(|alias| alias.get(NAME_FIELD))
                        && !names.contains(name)
                    {
                        names.push(name.clone());
                    }
                }
            }
            fields
                .values()
                .fold(names, |names, child| collect_cte_names(child, names))
        }
        Value::Array(items) => items
            .iter()
            .fold(names, |names, item| collect_cte_names(item, names)),
        _ => names,
    }
}

/// The keys of the file's CTEs that `sql` reads, in reading order.
fn known_cte_references(
    parser: Option<&dyn NativeDialect>,
    sql: &str,
    names_by_key: &HashMap<String, &str>,
    syntax: &LexicalSyntax,
) -> Result<Vec<String>, String> {
    let folded: String = casefold(sql);
    if !names_by_key.keys().any(|key| folded.contains(key.as_str())) {
        return Ok(Vec::new());
    }
    let Some(parsed) = parse_one(parser, sql) else {
        return identifier_references(sql, names_by_key, syntax);
    };
    let mut references: Vec<String> = Vec::new();
    for node in parsed.dfs().skip(1) {
        if let Expression::Table(table) = node {
            references = push_known(references, casefold(&table.name.name), names_by_key);
        }
    }
    Ok(references)
}

/// The identifier scan used when the project's parser cannot read a body.
fn identifier_references(
    sql: &str,
    names_by_key: &HashMap<String, &str>,
    syntax: &LexicalSyntax,
) -> Result<Vec<String>, String> {
    let bytes: &[u8] = sql.as_bytes();
    let mut references: Vec<String> = Vec::new();
    let mut index: usize = 0;
    while let Some(character) = sql[index..].chars().next() {
        match dialect_non_code_end(bytes, index, syntax) {
            Err(construct) => return Err(unclosed_message(construct)),
            Ok(Some(end)) if IDENTIFIER_QUOTES.contains(&bytes[index]) => {
                let quote: &str = &sql[index..=index];
                let name: String = sql[index + 1..end - 1].replace(&quote.repeat(2), quote);
                references = push_known(references, casefold(&name), names_by_key);
                index = end;
            }
            Ok(Some(end)) => index = end,
            Ok(None) if character.is_alphabetic() || character == '_' => {
                let end: usize = sql[index..]
                    .char_indices()
                    .skip(1)
                    .find(|(_, next)| !(next.is_alphanumeric() || *next == '_'))
                    .map_or(sql.len(), |(offset, _)| index + offset);
                references = push_known(references, casefold(&sql[index..end]), names_by_key);
                index = end;
            }
            Ok(None) => index += character.len_utf8(),
        }
    }
    Ok(references)
}

/// `references` with `key` appended when it names a file CTE not yet read.
fn push_known(
    mut references: Vec<String>,
    key: String,
    names_by_key: &HashMap<String, &str>,
) -> Vec<String> {
    if names_by_key.contains_key(&key) && !references.contains(&key) {
        references.push(key);
    }
    references
}

fn unclosed_message(construct: Unclosed) -> String {
    let construct: &str = match construct {
        Unclosed::BlockComment => "block comment",
        Unclosed::Quote => "quoted string",
        Unclosed::Parenthesis => "parenthesis",
    };
    format!("SQL test contains an unclosed {construct}")
}

/// The shortest dependency path from `origin` to a prohibited CTE, breadth first.
fn dependency_path(
    origin: &str,
    prohibited: &BTreeSet<String>,
    dependencies: &HashMap<String, Vec<String>>,
) -> Option<Vec<String>> {
    breadth_first_path(
        origin.to_owned(),
        (
            |last: &String| dependencies.get(last).into_iter().flatten().cloned(),
            |dependency: &String| prohibited.contains(dependency),
            |_: &String| true,
        ),
    )
}
