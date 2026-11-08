//! Databricks' native module: SQLQuack's Databricks SQL parser and its DATABRICKS profile.

use sqlquack_semantics::dialects::constants::DATABRICKS;
use sqlquack_semantics::dialects::types::DialectProfile;
use sqlquack_syntax_databricks::{Parsed, StrictFailure, parse_strict};

use crate::analysis_route::constants::{
    DATABRICKS_DIALECT, DATABRICKS_STUB_PREFIX, DATABRICKS_TYPE_ALIASES, UNSCANNED_CONSTRUCT,
};
use crate::analysis_route::models::{DatabricksDialect, ParserGap};
use crate::analysis_route::types::NativeDialect;
use crate::type_system::_helpers::fallback::normalize_with_fallback;
use crate::type_system::models::{NormalizedType, TypeDialect};

impl NativeDialect for DatabricksDialect {
    fn name(&self) -> &'static str {
        DATABRICKS_DIALECT
    }

    fn stub_prefix(&self) -> &'static str {
        DATABRICKS_STUB_PREFIX
    }

    fn parse(&self, sql: &str) -> Result<Parsed, ParserGap> {
        parse_strict(sql).map_err(|failure| match failure {
            StrictFailure::Parse(error) => ParserGap {
                construct: error.message.clone(),
                message: error.message,
                offset: error.position,
                syntax: error.syntax,
            },
            StrictFailure::Uncovered { position } => ParserGap {
                construct: UNSCANNED_CONSTRUCT.to_owned(),
                message: UNSCANNED_CONSTRUCT.to_owned(),
                offset: position,
                syntax: false,
            },
        })
    }

    fn profile(&self) -> &'static dyn DialectProfile {
        &DATABRICKS
    }

    fn normalize_type(&self, spelling: &str) -> Option<NormalizedType> {
        let upper: String = spelling
            .split_whitespace()
            .collect::<Vec<&str>>()
            .join(" ")
            .to_ascii_uppercase();
        if upper.is_empty() {
            return None;
        }
        let spelt: String = DATABRICKS_TYPE_ALIASES
            .iter()
            .fold(upper, |text, (alias, name)| aliased(&text, alias, name));
        Some(normalize_with_fallback(
            &spelt,
            Some(TypeDialect::Databricks),
        ))
    }
}

/// `text` with each whole word `alias` spelt `name`, at any nesting depth.
fn aliased(text: &str, alias: &str, name: &str) -> String {
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut out: String = String::with_capacity(text.len());
    let mut rest: &str = text;
    while let Some(at) = rest.find(alias) {
        let before: Option<char> = rest[..at].chars().next_back().or(out.chars().next_back());
        let after: Option<char> = rest[at + alias.len()..].chars().next();
        out.push_str(&rest[..at]);
        match before.is_some_and(word) || after.is_some_and(word) {
            true => out.push_str(alias),
            false => out.push_str(name),
        }
        rest = &rest[at + alias.len()..];
    }
    out.push_str(rest);
    out
}
