//! `neutralize_interpolation`: SQLBuild references, macros and templates replaced by sentinels.

use regex::Regex;
use sqlbuild_core::text::main::python_isspace::python_isspace;
use sqlbuild_sqltext::sql_scan::main::follows_table_wrapper::follows_table_wrapper;
use sqlbuild_sqltext::sql_scan::main::matching_paren::matching_paren;
use sqlbuild_sqltext::sql_scan::models::QuotePolicy;

use crate::lint_text::_helpers::sites::backtick_identifiers;
use crate::lint_text::_helpers::sites::{
    ByteSite, assemble, lint_policy, sentinel, site_end, site_pattern, site_starts,
};
use crate::lint_text::constants::{LATERAL_KEYWORD, LINT_SENTINEL_PREFIX};
use crate::lint_text::errors::LintTextError;
use crate::lint_text::models::Neutralized;
use crate::query_facts::constants::SNOWFLAKE_DIALECT;

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
        if dialect == SNOWFLAKE_DIALECT {
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

/// The end of a `__table_fn(..)` call with its arguments, outside `TABLE( )` and `LATERAL`.
fn unwrapped_table_call_end(
    body: &str,
    start: usize,
    end: usize,
    policy: QuotePolicy,
) -> Option<usize> {
    let before: &[u8] = &body.as_bytes()[..start];
    if !body[start..end].to_lowercase().starts_with("__table_fn")
        || follows_table_wrapper(before)
        || follows_word(before, LATERAL_KEYWORD)
    {
        return None;
    }
    let open: usize = body.len() - body[end..].trim_start_matches(python_isspace).len();
    if body.as_bytes().get(open) != Some(&b'(') {
        return None;
    }
    match matching_paren(body.as_bytes(), open, policy) {
        Ok(close) => Some(close + 1),
        Err(_) => None,
    }
}

/// Whether `before`, ignoring trailing ASCII whitespace, ends with the whole word `word`.
fn follows_word(before: &[u8], word: &str) -> bool {
    let before: &[u8] = before.trim_ascii_end();
    let Some(start) = before.len().checked_sub(word.len()) else {
        return false;
    };
    before[start..].eq_ignore_ascii_case(word.as_bytes())
        && start.checked_sub(1).is_none_or(|index| {
            !(before[index].is_ascii_alphanumeric() || matches!(before[index], b'_' | b'$'))
        })
}
