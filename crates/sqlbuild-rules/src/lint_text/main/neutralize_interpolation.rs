//! `neutralize_interpolation`: SQLBuild references, macros and templates replaced by sentinels.

use regex::Regex;
use sqlbuild_core::text::main::python_isspace::python_isspace;
use sqlbuild_sqltext::sql_scan::main::matching_paren::matching_paren;
use sqlbuild_sqltext::sql_scan::models::QuotePolicy;

use crate::lint_text::_helpers::sites::backtick_identifiers;
use crate::lint_text::_helpers::sites::{
    ByteSite, assemble, lint_policy, sentinel, site_end, site_pattern, site_starts,
};
use crate::lint_text::constants::LINT_SENTINEL_PREFIX;
use crate::lint_text::errors::LintTextError;
use crate::lint_text::models::Neutralized;

/// Neutralize references for lint, or every intrinsic spelling in any case for formatting; `@'name'` becomes a string.
pub fn neutralize_interpolation(
    body: &str,
    dialect: &str,
    formatting: bool,
) -> Result<Neutralized, LintTextError> {
    let pattern: &Regex = site_pattern(formatting)?;
    let policy = lint_policy(backtick_identifiers(dialect));
    let mut byte_sites: Vec<ByteSite> = Vec::new();
    let mut copied_to: usize = 0;
    for start in site_starts(body, pattern, policy) {
        if start < copied_to {
            continue;
        }
        let Some(mut end) = site_end(body, start, policy) else {
            continue;
        };
        if dialect == "snowflake" {
            end = unwrapped_table_call_end(body, start, end, policy).unwrap_or(end);
        }
        let identifier: String = sentinel(LINT_SENTINEL_PREFIX, byte_sites.len());
        let replacement: String = if body[start..].starts_with("@'") {
            format!("'{identifier}'")
        } else {
            identifier
        };
        byte_sites.push((start, end, replacement));
        copied_to = end;
    }
    Ok(assemble(body, byte_sites))
}

/// The end of the arguments after a `__table_fn(..)` site outside `TABLE( )`.
///
/// Snowflake reads a table function call only inside `TABLE( )`, so the whole call stands as one
/// relation; a call the author already wrapped keeps its arguments.
fn unwrapped_table_call_end(
    body: &str,
    start: usize,
    end: usize,
    policy: QuotePolicy,
) -> Option<usize> {
    if !body[start..end].to_lowercase().starts_with("__table_fn") {
        return None;
    }
    let wrapped: bool = body[..start]
        .trim_end_matches(python_isspace)
        .strip_suffix('(')
        .map(|before| before.trim_end_matches(python_isspace).to_lowercase())
        .is_some_and(|before| before.ends_with("table"));
    if wrapped {
        return None;
    }
    let open: usize = body.len() - body[end..].trim_start_matches(python_isspace).len();
    (body.as_bytes().get(open) == Some(&b'('))
        .then(|| matching_paren(body.as_bytes(), open, policy).ok())
        .flatten()
        .map(|close| close + 1)
}
