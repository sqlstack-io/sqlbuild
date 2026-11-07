//! One body's Polyglot and SQLQuack SQL Server parses, and how their verdicts compare.

use std::collections::HashMap;

use polyglot_sql::{ComplexityGuardOptions, Dialect, DialectType, Expression, ParseOptions};
use serde::Serialize;
use sqlbuild_analysis::assembly::project::main::sql_syntax_error::sql_syntax_error;
use sqlbuild_analysis::assembly::project::models::{SyntaxCheck, SyntaxMode};
use sqlbuild_analysis::semantic_validation::main::normalize::normalize_analysis_sql;
use sqlbuild_analysis::semantic_validation::models::NormalizationInput;
use sqlquack_syntax_sqlserver::scan::main::scan::scan;
use sqlquack_syntax_sqlserver::scan::models::{Token, TokenKind};
use sqlquack_syntax_sqlserver::{Parsed, StrictFailure, parse_strict};

use crate::cases::Case;

const DIALECT: &str = "tsql";
/// SQLBuild's syntax gate bounds Polyglot's call depth (`MAX_FUNCTION_CALL_DEPTH`).
const MAX_FUNCTION_CALL_DEPTH: usize = 512;
/// Leading words that name a query rather than a statement kind.
const QUERY_WORDS: [&str; 6] = ["SELECT", "WITH", "FROM", "VALUES", "TABLE", "("];
/// Words between `CREATE` and the object kind.
const CREATE_MODIFIERS: [&str; 6] = ["OR", "REPLACE", "TEMP", "TEMPORARY", "PERSISTENT", "UNIQUE"];

/// One body's comparison.
#[derive(Debug, Serialize)]
pub struct Comparison {
    pub id: String,
    pub source: String,
    pub kind: String,
    pub mode: String,
    /// The body as Polyglot saw it, after SQLBuild's normalization.
    pub cleaned: String,
    /// The text SQLQuack parsed when it is not `cleaned`: the statement SQL Server receives.
    pub sqlquack_input: Option<String>,
    /// The statement forms of the cleaned body, in order.
    pub forms: Vec<String>,
    /// `normalized`, or the error SQLBuild's normalization raised (the raw text is then parsed).
    pub normalization: String,
    /// SQLBuild's own syntax gate for the body's mode: `accept`, `reject` or `none`.
    pub gate: String,
    pub gate_message: Option<String>,
    /// `structured`, `opaque` (some statement is a Polyglot `Command`) or `error`.
    pub polyglot: String,
    pub polyglot_statements: usize,
    pub polyglot_message: Option<String>,
    /// `tree`, `gap` or `error`.
    pub sqlquack: String,
    pub sqlquack_statements: usize,
    pub sqlquack_gaps: Vec<String>,
    pub sqlquack_message: Option<String>,
    /// Whether SQLQuack's refusal is the grammar's or the scanner's, not a transformer check.
    pub sqlquack_syntax: Option<bool>,
    /// `both`, `polyglot_only`, `sqlquack_only` or `neither`; SQLQuack parses only to a `tree`.
    pub category: String,
}

pub fn panicked(case: &Case) -> Comparison {
    let mut comparison: Comparison = empty(case);
    comparison.polyglot_message = Some("the harness panicked".to_owned());
    comparison
}

fn empty(case: &Case) -> Comparison {
    Comparison {
        id: case.id.clone(),
        source: case.source.clone(),
        kind: case.kind.clone(),
        mode: case.mode.clone(),
        cleaned: String::new(),
        sqlquack_input: None,
        forms: Vec::new(),
        normalization: String::new(),
        gate: "none".to_owned(),
        gate_message: None,
        polyglot: "error".to_owned(),
        polyglot_statements: 0,
        polyglot_message: None,
        sqlquack: "error".to_owned(),
        sqlquack_statements: 0,
        sqlquack_gaps: Vec::new(),
        sqlquack_message: None,
        sqlquack_syntax: None,
        category: "neither".to_owned(),
    }
}

pub fn compare_case(case: &Case) -> Comparison {
    let mut comparison: Comparison = empty(case);
    let cleaned: String = match normalized(&case.sql, &case.placeholders) {
        Ok(cleaned) => {
            comparison.normalization = "normalized".to_owned();
            cleaned
        }
        Err(error) => {
            comparison.normalization = error;
            case.sql.clone()
        }
    };
    comparison.forms = forms(&cleaned);
    comparison.cleaned.clone_from(&cleaned);
    gate(case, &mut comparison);
    let guard = ComplexityGuardOptions {
        max_function_call_depth: Some(MAX_FUNCTION_CALL_DEPTH),
        ..Default::default()
    };
    let options = ParseOptions {
        complexity_guard: Some(guard),
    };
    match Dialect::get(DialectType::TSQL).parse_with_options(&cleaned, &options) {
        Ok(statements) => {
            let opaque: bool = statements
                .iter()
                .any(|statement| matches!(statement, Expression::Command(_)));
            comparison.polyglot = if opaque { "opaque" } else { "structured" }.to_owned();
            comparison.polyglot_statements = statements.len();
        }
        Err(error) => comparison.polyglot_message = Some(error.to_string()),
    }
    let sqlquack_input: String = match &case.sqlquack_sql {
        Some(wrapped) => cleaned_text(wrapped, &case.placeholders),
        None => cleaned.clone(),
    };
    if sqlquack_input != cleaned {
        comparison.sqlquack_input = Some(sqlquack_input.clone());
    }
    match parse_strict(&sqlquack_input) {
        Ok(Parsed { statements, gaps }) => {
            comparison.sqlquack = if gaps.is_empty() { "tree" } else { "gap" }.to_owned();
            comparison.sqlquack_statements = statements.len();
            comparison.sqlquack_gaps = gaps.iter().map(|gap| gap.construct.to_owned()).collect();
        }
        Err(StrictFailure::Parse(error)) => {
            comparison.sqlquack_message = Some(error.message);
            comparison.sqlquack_syntax = Some(error.syntax);
        }
    }
    let polyglot: bool = comparison.polyglot != "error";
    // A gap is SQLQuack declining to decide, so only a gap-free tree counts as a parse.
    let sqlquack: bool = comparison.sqlquack == "tree";
    comparison.category = match (polyglot, sqlquack) {
        (true, true) => "both",
        (true, false) => "polyglot_only",
        (false, true) => "sqlquack_only",
        (false, false) => "neither",
    }
    .to_owned();
    comparison
}

/// `sql` as SQLBuild's normalization leaves it for the parser.
fn normalized(sql: &str, placeholders: &HashMap<String, String>) -> Result<String, String> {
    normalize_analysis_sql(NormalizationInput {
        sql: sql.to_owned(),
        dialect: DIALECT.to_owned(),
        stubs: HashMap::new(),
        placeholders: placeholders.clone(),
    })
}

/// `sql` normalized, or as written when normalization refuses it.
fn cleaned_text(sql: &str, placeholders: &HashMap<String, String>) -> String {
    normalized(sql, placeholders).unwrap_or_else(|_| sql.to_owned())
}

/// SQLBuild's own gate for bodies it checks: `validate` for model queries and hooks, `parse`
/// (one statement) for function bodies.
fn gate(case: &Case, comparison: &mut Comparison) {
    let mode: SyntaxMode = match case.mode.as_str() {
        "validate" => SyntaxMode::Validate,
        "parse" => SyntaxMode::Parse,
        _ => return,
    };
    let check: SyntaxCheck = SyntaxCheck {
        sql: case.sql.clone(),
        placeholders: case
            .placeholders
            .iter()
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect(),
    };
    match sql_syntax_error(&check, DIALECT, mode) {
        Ok(None) => comparison.gate = "accept".to_owned(),
        Ok(Some(message)) => {
            comparison.gate = "reject".to_owned();
            comparison.gate_message = Some(message);
        }
        Err(failure) => {
            comparison.gate = "reject".to_owned();
            comparison.gate_message = Some(format!("{failure:?}"));
        }
    }
}

/// The form of each statement of `sql`: `QUERY`, or its leading words such as `CREATE TABLE`.
fn forms(sql: &str) -> Vec<String> {
    let tokens: Vec<Token> = scan(sql);
    let mut forms: Vec<String> = Vec::new();
    let mut depth: usize = 0;
    let mut start: Option<usize> = None;
    for (index, token) in tokens.iter().enumerate() {
        match token.kind {
            TokenKind::Char(';') if depth == 0 => {
                if let Some(first) = start.take() {
                    forms.push(form(&tokens[first..index]));
                }
                continue;
            }
            TokenKind::Char('(') => depth += 1,
            TokenKind::Char(')') => depth = depth.saturating_sub(1),
            _ => {}
        }
        start.get_or_insert(index);
    }
    if let Some(first) = start {
        forms.push(form(&tokens[first..]));
    }
    forms
}

/// The form of one statement's tokens.
fn form(tokens: &[Token]) -> String {
    let words: Vec<String> = tokens
        .iter()
        .take(8)
        .map(|token| match token.kind {
            TokenKind::Word => token.value.to_ascii_uppercase(),
            _ => token.value.clone(),
        })
        .collect();
    let Some(first) = words.first() else {
        return "EMPTY".to_owned();
    };
    if QUERY_WORDS.contains(&first.as_str()) {
        return "QUERY".to_owned();
    }
    let rest: Vec<&String> = words[1..]
        .iter()
        .filter(|word| !(first == "CREATE" && CREATE_MODIFIERS.contains(&word.as_str())))
        .collect();
    match first.as_str() {
        "CREATE" | "DROP" | "ALTER" | "COMMENT" | "ATTACH" | "DETACH" | "EXPORT" | "IMPORT"
        | "INSERT" | "COPY" | "SHOW" | "SET" | "RESET" | "PRAGMA" => match rest.first() {
            Some(second) if second.chars().all(|c| c.is_ascii_alphabetic()) => {
                format!("{first} {second}")
            }
            _ => first.clone(),
        },
        _ => first.clone(),
    }
}
