//! `neutralize_interpolation`: SQLBuild references, macros and templates replaced by sentinels.

use regex::Regex;

use crate::lint_text::_helpers::sites::backtick_identifiers;
use crate::lint_text::_helpers::sites::{
    ByteSite, assemble, lint_policy, sentinel, site_end, site_pattern, site_starts,
    unwrapped_table_function_end,
};
use crate::lint_text::constants::{
    LINT_SENTINEL_PREFIX, TABLE_FUNCTION_WRAPPER, TABLE_FUNCTION_WRAPPING_DIALECT,
};
use crate::lint_text::errors::LintTextError;
use crate::lint_text::models::Neutralized;

/// Neutralize references for lint or every intrinsic for formatting; `@'name'` becomes a string, a Snowflake table function call `TABLE(<sentinel>())`.
pub fn neutralize_interpolation(
    body: &str,
    dialect: &str,
    formatting: bool,
) -> Result<Neutralized, LintTextError> {
    let pattern: &Regex = site_pattern(formatting)?;
    let policy = lint_policy(backtick_identifiers(dialect));
    let wraps_table_functions: bool = dialect == TABLE_FUNCTION_WRAPPING_DIALECT;
    let mut byte_sites: Vec<ByteSite> = Vec::new();
    let mut copied_to: usize = 0;
    for start in site_starts(body, pattern, policy) {
        if start < copied_to {
            continue;
        }
        let Some(mut end) = site_end(body, start, policy) else {
            continue;
        };
        let identifier: String = sentinel(LINT_SENTINEL_PREFIX, byte_sites.len());
        let wrapped_call: Option<usize> = wraps_table_functions
            .then(|| unwrapped_table_function_end(body, (start, end), policy))
            .flatten();
        let replacement: String = if let Some(call_end) = wrapped_call {
            end = call_end;
            format!("{TABLE_FUNCTION_WRAPPER}{identifier}())")
        } else if body[start..].starts_with("@'") {
            format!("'{identifier}'")
        } else {
            identifier
        };
        byte_sites.push((start, end, replacement));
        copied_to = end;
    }
    Ok(assemble(body, byte_sites))
}
