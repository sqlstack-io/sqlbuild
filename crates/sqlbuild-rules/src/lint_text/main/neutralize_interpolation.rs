//! `neutralize_interpolation`: SQLBuild references, macros and templates replaced by sentinels.

use regex::Regex;

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
        let Some(end) = site_end(body, start, policy) else {
            continue;
        };
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
